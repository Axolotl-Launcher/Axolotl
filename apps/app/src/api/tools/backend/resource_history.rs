use super::{LauncherBackend, business_error, jobs, page, resources};
use launcher_tools::{BackendContext, Command, FavoriteKind, ToolError};
use serde_json::{Value, json};
use theseus::resources::{self as r, *};
use tokio_util::sync::CancellationToken;

fn favorite_kind(kind: FavoriteKind) -> ResourceType {
    match kind {
        FavoriteKind::Mod => ResourceType::Mod,
        FavoriteKind::ResourcePack => ResourceType::ResourcePack,
        FavoriteKind::DataPack => ResourceType::DataPack,
        FavoriteKind::Shader => ResourceType::Shader,
    }
}
fn identifier<T>(value: String) -> Result<T, ToolError>
where
    T: TryFrom<String>,
{
    T::try_from(value).map_err(|_| {
        ToolError::new(
            "invalid_identifier",
            "Use the exact identifier returned by the resource tools",
        )
    })
}
pub(super) async fn idle(id: &str) -> Result<(), ToolError> {
    if theseus::instance::get(id)
        .await
        .map_err(business_error)?
        .is_none()
    {
        return Err(ToolError::new("not_found", "Instance not found"));
    }
    if !theseus::process::get_by_instance_id(id)
        .await
        .map_err(business_error)?
        .is_empty()
    {
        return Err(ToolError::new(
            "instance_busy",
            "Stop the game before changing instance content",
        ));
    }
    if theseus::install::list_jobs(false)
        .await
        .map_err(business_error)?
        .iter()
        .any(|job| {
            !job.status.is_finished()
                && (job.instance_id.as_deref() == Some(id)
                    || job.source_instance_id.as_deref() == Some(id))
        })
        || !theseus::instance::list_backup_operations(Some(id), true)
            .await
            .map_err(business_error)?
            .is_empty()
    {
        return Err(ToolError::new(
            "instance_busy",
            "Wait for active instance jobs/backups before resource maintenance",
        ));
    }
    Ok(())
}
fn favorite_summary(favorite: r::favorites::ResourceFavorite) -> Value {
    json!({"source":favorite.project.source,"project_id":favorite.project.project_id,"resource_type":favorite.content_type,"saved_at":favorite.saved_at,
        "confirmed_projects":favorite.projects.into_iter().take(20).map(|project|json!({"source":project.source,"project_id":project.project_id})).collect::<Vec<_>>()})
}
fn installed_summary(item: InstalledResource) -> Value {
    json!({"entry_id":item.entry_id,"relative_path":item.relative_path,"artifact":item.artifact,"enabled":item.enabled,"missing":item.missing,"resource_type":item.resource_type,"scope":item.scope})
}
fn history_summary(entry: ResourceHistoryEntry) -> Value {
    json!({"history_id":entry.id,"entry_id":entry.entry_id,"instance_id":entry.instance_id,"recorded_at":entry.recorded_at,
        "previous":installed_summary(entry.previous),"target":installed_summary(entry.target),"backup_available":entry.backup_available,"backup_retired":entry.backup_retired})
}
fn backup_summary(file: ResourceBackupFile) -> Value {
    json!({"backup_id":file.id,"history_id":file.history_id,"relative_path":file.relative_path,"size_bytes":file.integrity.size,
        "rollback_available":file.rollback_available,"recognized":file.recognized,"exists":file.exists})
}
fn review(
    backend: &LauncherBackend,
    session: &str,
    plan: ResourceHistoryCleanupPlan,
) -> Result<Value, ToolError> {
    if plan.files.is_empty()
        || plan.files.len() > 100
        || plan.issues.len() > 100
    {
        return Err(ToolError::new(
            "cleanup_plan_too_large",
            "Choose between 1 and 100 backups and prepare a bounded cleanup plan",
        ));
    }
    let executable = !plan.issues.iter().any(|issue| issue.blocking);
    let bytes = plan.files.iter().fold(0u64, |size, file| {
        size.saturating_add(file.integrity.size.unwrap_or(0))
    });
    let handle = backend.resource_cleanups.insert(
        session,
        plan.revision,
        plan.clone(),
    )?;
    Ok(
        json!({"plan_id":handle,"revision":plan.revision,"instance_id":plan.instance_id,"executable":executable,
        "file_count":plan.files.len(),"total_size_bytes":bytes,"files":plan.files.into_iter().map(backup_summary).collect::<Vec<_>>(),
        "issues":plan.issues.into_iter().map(|issue|json!({"code":issue.code,"blocking":issue.blocking,"message":issue.message.chars().take(512).collect::<String>()})).collect::<Vec<_>>()}),
    )
}

pub(super) async fn execute(
    backend: &LauncherBackend,
    command: Command,
    context: BackendContext,
) -> Result<Value, ToolError> {
    match command {
        Command::ResourceFavoritesList { offset, limit } => {
            let mut favorites =
                r::favorites::list().await.map_err(resources::error)?;
            favorites.sort_by(|a, b| {
                b.saved_at
                    .cmp(&a.saved_at)
                    .then(a.project.source.cmp(&b.project.source))
                    .then(a.project.project_id.cmp(&b.project.project_id))
            });
            page(
                favorites.into_iter().map(favorite_summary).collect(),
                offset,
                limit,
            )
        }
        Command::ResourceFavoriteAdd {
            source,
            project_id,
            resource_type,
        } => {
            let project = resources::project(source, project_id)?;
            let kind = favorite_kind(resource_type);
            let state = theseus::State::get().await.map_err(business_error)?;
            let service = state.resources().await.map_err(resources::error)?;
            let resource = service
                .get(
                    &ResourceRef::Source {
                        project: project.clone(),
                    },
                    None,
                    None,
                    &CancellationToken::new(),
                )
                .await
                .map_err(resources::error)?;
            if !resource.projects.iter().any(|metadata| {
                metadata.project == project && metadata.types.contains(&kind)
            }) {
                return Err(ToolError::new(
                    "favorite_type_mismatch",
                    "Project does not advertise this supported favorite type",
                ));
            }
            r::favorites::add(project.clone(), kind, None)
                .await
                .map_err(resources::error)?;
            Ok(
                json!({"saved":true,"source":project.source,"project_id":project.project_id,"resource_type":kind}),
            )
        }
        Command::ResourceFavoriteRemove { source, project_id } => {
            let project = resources::project(source, project_id)?;
            r::favorites::remove(&project)
                .await
                .map_err(resources::error)?;
            Ok(
                json!({"removed":true,"source":project.source,"project_id":project.project_id}),
            )
        }
        Command::ResourceHistoryList {
            instance_id,
            entry_id,
            offset,
            limit,
        } => {
            let entries = r::history::list(
                instance_id,
                entry_id.map(identifier::<InstalledEntryId>).transpose()?,
            )
            .await
            .map_err(resources::error)?;
            let mut entries = entries;
            entries.sort_by(|a, b| {
                b.recorded_at.cmp(&a.recorded_at).then(a.id.cmp(&b.id))
            });
            page(
                entries.into_iter().map(history_summary).collect(),
                offset,
                limit,
            )
        }
        Command::ResourceHistoryRollback {
            instance_id,
            entry_id,
            history_id,
        } => {
            idle(&instance_id).await?;
            Ok(jobs::summary(
                r::history::rollback(
                    instance_id,
                    identifier(entry_id)?,
                    identifier(history_id)?,
                )
                .await
                .map_err(resources::error)?,
            ))
        }
        Command::ResourceScanBackups {
            instance_id,
            offset,
            limit,
        } => {
            idle(&instance_id).await?;
            let mut files = r::history_cleanup::scan(instance_id)
                .await
                .map_err(resources::error)?;
            files.sort_by(|a, b| {
                a.relative_path.cmp(&b.relative_path).then(a.id.cmp(&b.id))
            });
            page(
                files.into_iter().map(backup_summary).collect(),
                offset,
                limit,
            )
        }
        Command::ResourcePrepareHistoryCleanup {
            instance_id,
            backup_ids,
        } => {
            idle(&instance_id).await?;
            let plan = r::history_cleanup::prepare(
                instance_id,
                backup_ids
                    .into_iter()
                    .map(identifier::<HistoryId>)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .await
            .map_err(resources::error)?;
            review(backend, &context.session_id, plan)
        }
        Command::ResourceExecuteHistoryCleanup { plan_id, revision } => {
            let plan = backend.resource_cleanups.get(
                &context.session_id,
                plan_id,
                revision,
            )?;
            if plan.issues.iter().any(|issue| issue.blocking) {
                return Err(ToolError::new(
                    "cleanup_plan_blocked",
                    "Prepare a fresh cleanup plan without blocking issues",
                ));
            }
            idle(&plan.instance_id).await?;
            let plan = backend.resource_cleanups.take(
                &context.session_id,
                plan_id,
                revision,
            )?;
            let result = r::history_cleanup::execute(plan.id, revision)
                .await
                .map_err(resources::error)?;
            Ok(
                json!({"removed_count":result.removed.len(),"removed":result.removed,"failure_count":result.failures.len(),
                "failures":result.failures.into_iter().take(100).map(|(id,_)|json!({"backup_id":id,"code":"cleanup_failed"})).collect::<Vec<_>>()}),
            )
        }
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a favorite/history operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn backup(id: &str, size: u64) -> ResourceBackupFile {
        ResourceBackupFile {
            id: HistoryId::new(id).unwrap(),
            history_id: None,
            relative_path: format!("mods/{id}.old"),
            original_file_name: Some("mod.jar".into()),
            integrity: ArtifactIntegrity {
                size: Some(size),
                hashes: vec![],
            },
            rollback_available: true,
            recognized: true,
            exists: true,
        }
    }
    fn plan() -> ResourceHistoryCleanupPlan {
        ResourceHistoryCleanupPlan {
            id: PlanId::new("private-native-plan").unwrap(),
            revision: 2,
            instance_id: "instance".into(),
            instance_revision: 1,
            entries: vec![],
            files: vec![backup("a", 100), backup("b", 200)],
            issues: vec![],
        }
    }
    #[test]
    fn cleanup_review_counts_files_and_keeps_native_plan_private() {
        let backend = LauncherBackend::default();
        let value = review(&backend, "owner", plan()).unwrap();
        assert_eq!(value["file_count"], 2);
        assert_eq!(value["total_size_bytes"], 300);
        assert_eq!(value["executable"], true);
        assert!(!value.to_string().contains("private-native-plan"));
        let id =
            uuid::Uuid::parse_str(value["plan_id"].as_str().unwrap()).unwrap();
        assert!(backend.resource_cleanups.get("other", id, 2).is_err());
        assert!(backend.resource_cleanups.get("owner", id, 1).is_err());
        assert_eq!(
            backend
                .resource_cleanups
                .take("owner", id, 2)
                .unwrap()
                .id
                .as_str(),
            "private-native-plan"
        );
        assert!(backend.resource_cleanups.take("owner", id, 2).is_err());
    }
    #[test]
    fn cleanup_review_requires_bounded_nonempty_file_selection() {
        let backend = LauncherBackend::default();
        let mut plan = plan();
        plan.files.clear();
        assert_eq!(
            review(&backend, "owner", plan.clone()).unwrap_err().code,
            "cleanup_plan_too_large"
        );
        plan.files = vec![backup("a", 1); 101];
        assert_eq!(
            review(&backend, "owner", plan).unwrap_err().code,
            "cleanup_plan_too_large"
        );
    }
    #[test]
    fn favorite_summaries_omit_locator_and_keep_confirmed_source_ids() {
        let favorite:r::favorites::ResourceFavorite=serde_json::from_value(json!({"locator":"https://private/locator","resourceId":"resource","project":{"source":"modrinth","projectId":"project"},
            "projects":[{"source":"modrinth","projectId":"project"},{"source":"curseforge","projectId":"123"}],"contentType":"mod","savedAt":10})).unwrap();
        let value = favorite_summary(favorite);
        assert_eq!(value["project_id"], "project");
        assert_eq!(value["confirmed_projects"].as_array().unwrap().len(), 2);
        assert!(!value.to_string().contains("private"));
        assert!(value.get("locator").is_none());
    }
    #[test]
    fn history_summaries_omit_native_backup_path_and_dependency_artifacts() {
        let installed = json!({"entryId":"entry","instanceId":"instance","resourceId":null,"artifact":null,"knownArtifacts":[],"integrity":{"size":100,"hashes":[]},"relativePath":"mods/mod.jar",
            "originalFileName":"mod.jar","resourceType":"mod","scope":{"kind":"instance"},"enabled":true,"missing":false,"ownership":{"kind":"userAdded"}});
        let entry:ResourceHistoryEntry=serde_json::from_value(json!({"id":"history","entryId":"entry","instanceId":"instance","recordedAt":"2026-01-01T00:00:00Z",
            "previous":installed,"target":installed,"previousNodes":[],"targetNodes":[],"previousDependencies":[],"targetDependencies":[],
            "backupRelativePath":"private-backup.old","backupIntegrity":{"size":100,"hashes":[]},"backupAvailable":true,"backupRetired":false})).unwrap();
        let value = history_summary(entry);
        assert_eq!(value["history_id"], "history");
        assert_eq!(value["backup_available"], true);
        assert!(!value.to_string().contains("private-backup"));
        assert!(value.get("previous_nodes").is_none());
    }
}
