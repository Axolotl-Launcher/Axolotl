use chrono::Utc;
use sqlx::Row;

use super::*;

fn db(error: sqlx::Error) -> ResourceError {
    ResourceError::Database(error.to_string())
}

pub async fn list(
    instance_id: String,
    entry_id: Option<InstalledEntryId>,
) -> ResourceResult<Vec<ResourceHistoryEntry>> {
    let state = crate::State::get().await?;
    list_with_state(&instance_id, entry_id.as_ref(), &state).await
}

pub(crate) async fn list_with_state(
    instance_id: &str,
    entry_id: Option<&InstalledEntryId>,
    state: &crate::State,
) -> ResourceResult<Vec<ResourceHistoryEntry>> {
    read_history(instance_id, entry_id, state, true).await
}

async fn read_history(
    instance_id: &str,
    entry_id: Option<&InstalledEntryId>,
    state: &crate::State,
    verify_backups: bool,
) -> ResourceResult<Vec<ResourceHistoryEntry>> {
    let rows = if let Some(entry_id) = entry_id {
        sqlx::query("SELECT payload, backup_available FROM resource_history WHERE instance_id = ? AND entry_id = ? ORDER BY recorded_at DESC, rowid DESC")
            .bind(instance_id)
            .bind(entry_id.as_str())
            .fetch_all(&state.pool)
            .await
            .map_err(db)?
    } else {
        sqlx::query("SELECT payload, backup_available FROM resource_history WHERE instance_id = ? ORDER BY recorded_at DESC, rowid DESC")
            .bind(instance_id)
            .fetch_all(&state.pool)
            .await
            .map_err(db)?
    };
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &scope.instance,
    )
    .map_err(|error| ResourceError::InvalidRequest(error.to_string()))?;
    let mut entries = Vec::new();
    for row in rows {
        let Ok(mut entry) =
            serde_json::from_str::<ResourceHistoryEntry>(row.get("payload"))
        else {
            continue;
        };
        entry.backup_retired = row.get::<i64, _>("backup_available") == 0;
        entry.backup_available =
            match (&entry.backup_relative_path, &entry.backup_integrity) {
                (Some(path), Some(integrity))
                    if entry.backup_available
                        && row.get::<i64, _>("backup_available") != 0 =>
                {
                    if !verify_backups {
                        true
                    } else {
                        match super::execution::checked_path(&root, path) {
                            Ok(path) => {
                                super::execution::matches_file(&path, integrity)
                                    .await
                                    .unwrap_or(false)
                            }
                            Err(_) => false,
                        }
                    }
                }
                _ => false,
            };
        entries.push(entry);
    }
    Ok(entries)
}

pub(crate) async fn attach_to_snapshot(
    instance_id: &str,
    items: &mut [crate::state::InstanceContentSnapshotItem],
    state: &crate::State,
) -> ResourceResult<()> {
    let rows: Vec<(String, i64)> = sqlx::query_as("SELECT payload, backup_available FROM (SELECT payload, backup_available, ROW_NUMBER() OVER (PARTITION BY entry_id ORDER BY recorded_at DESC, rowid DESC) AS rank FROM resource_history WHERE instance_id = ?) WHERE rank = 1").bind(instance_id).fetch_all(&state.pool).await.map_err(db)?;
    let histories: Vec<ResourceHistoryEntry> = rows
        .into_iter()
        .filter_map(|(payload, available)| {
            let mut history: ResourceHistoryEntry =
                serde_json::from_str(&payload).ok()?;
            history.backup_available &= available != 0;
            Some(history)
        })
        .collect();
    if histories.is_empty() {
        return Ok(());
    }
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &scope.instance,
    )
    .map_err(|error| ResourceError::InvalidRequest(error.to_string()))?;
    let installed = plans::installed(&state.pool, instance_id).await?;
    let installed: std::collections::HashMap<_, _> = installed
        .into_iter()
        .map(|entry| (entry.entry_id.clone(), entry))
        .collect();
    let histories: std::collections::HashMap<_, _> = histories
        .into_iter()
        .map(|history| (history.entry_id.clone(), history))
        .collect();
    for item in items {
        let Some(current) = item
            .entry_id
            .as_ref()
            .and_then(|id| InstalledEntryId::new(id).ok())
            .and_then(|id| installed.get(&id))
        else {
            continue;
        };
        let Some(content) = item.content.as_mut() else {
            continue;
        };
        if histories.contains_key(&current.entry_id) {
            content.rollback = None;
        }
        if let Some(history) =
            histories.get(&current.entry_id).filter(|history| {
                history.backup_available
                    && current.artifact == history.target.artifact
                    && current.relative_path == history.target.relative_path
                    && identity::exact_integrity(
                        &current.integrity,
                        &history.target.integrity,
                    )
            })
        {
            let (Some(backup), Some(integrity)) =
                (&history.backup_relative_path, &history.backup_integrity)
            else {
                continue;
            };
            let Ok(backup) = super::execution::checked_path(&root, backup)
            else {
                continue;
            };
            if !super::execution::matches_file(&backup, integrity)
                .await
                .unwrap_or(false)
            {
                continue;
            }
            content.rollback = Some(crate::state::ContentItemRollback {
                file_name: history
                    .previous
                    .original_file_name
                    .clone()
                    .unwrap_or_else(|| history.previous.relative_path.clone()),
            });
        }
    }
    Ok(())
}

pub async fn rollback(
    instance_id: String,
    entry_id: InstalledEntryId,
    history_id: HistoryId,
) -> ResourceResult<crate::install::InstallJobSnapshot> {
    let state = crate::State::get().await?;
    let draft =
        prepare_rollback(instance_id.clone(), entry_id, history_id, &state)
            .await?;
    let id = draft.plan.id.clone();
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, ?, 1, ?)").bind(id.as_str()).bind(&instance_id).bind(serde_json::to_string(&draft)?).execute(&state.pool).await.map_err(db)?;
    plans::queue(id, 1).await
}

pub(crate) async fn prepare_rollback(
    instance_id: String,
    entry_id: InstalledEntryId,
    history_id: HistoryId,
    state: &crate::State,
) -> ResourceResult<plans::OperationDraft> {
    let histories =
        list_with_state(&instance_id, Some(&entry_id), state).await?;
    let history = histories
        .iter()
        .find(|entry| entry.id == history_id && entry.backup_available)
        .ok_or_else(|| {
            ResourceError::InvalidRequest(
                "verified rollback backup not available".into(),
            )
        })?;
    let payload: String =
        sqlx::query_scalar("SELECT payload FROM resource_history WHERE id = ?")
            .bind(history_id.as_str())
            .fetch_one(&state.pool)
            .await
            .map_err(db)?;
    let payload: serde_json::Value = serde_json::from_str(&payload)?;
    let record: super::installed::InstalledArtifactRecord =
        serde_json::from_value(
            payload.get("previousArtifact").cloned().ok_or_else(|| {
                ResourceError::InvalidRequest(
                    "history lacks exact artifact metadata".into(),
                )
            })?,
        )?;
    let artifact = record.artifact;
    if !identity::exact_integrity(
        &artifact.integrity,
        &history.previous.integrity,
    ) || history.previous.artifact.as_ref() != Some(&artifact.reference)
        || !plans::safe_relative_path(&history.previous.relative_path)
    {
        return Err(ResourceError::InvalidRequest(
            "rollback metadata disagrees with the verified backup".into(),
        ));
    }
    let current = plans::installed(&state.pool, &instance_id)
        .await?
        .into_iter()
        .find(|entry| entry.entry_id == entry_id && !entry.missing)
        .ok_or_else(|| {
            ResourceError::InvalidRequest("installed entry disappeared".into())
        })?;
    if !identity::exact_integrity(&current.integrity, &history.target.integrity)
        || current.artifact != history.target.artifact
        || current.relative_path != history.target.relative_path
    {
        return Err(ResourceError::InvalidRequest(
            "installed artifact changed since this history entry".into(),
        ));
    }
    let id = PlanId::new(uuid::Uuid::new_v4().to_string())?;
    let node_id = SelectionId::new(entry_id.as_str())?;
    let reference = current
        .resource_id
        .as_ref()
        .map(|id| ResourceRef::Resource { id: id.clone() })
        .unwrap_or_else(|| ResourceRef::Source {
            project: artifact.reference.release.project.clone(),
        });
    let plan = ResourceOperationPlan {
        schema_version: ResourceContractVersion,
        id: id.clone(),
        revision: 1,
        instance_id: instance_id.clone(),
        instance_revision: plans::instance_revision(&state.pool, &instance_id)
            .await?,
        intent: ResourceOperationIntent::SwitchVersion,
        selections: vec![ResourceSelection {
            id: node_id.clone(),
            resource: reference.clone(),
            release: ReleaseSelection::Artifact {
                reference: artifact.reference.clone(),
            },
            resource_type: current.resource_type,
            scope: current.scope.clone(),
            entry_id: Some(entry_id.clone()),
        }],
        active_roots: vec![node_id.clone()],
        nodes: vec![ResourceDependencyNode {
            id: node_id.clone(),
            root_selection_ids: vec![node_id.clone()],
            resource_type: Some(current.resource_type),
            resource: reference,
            scope: current.scope.clone(),
            state: DependencyNodeState::Update,
            artifact: Some(artifact.clone()),
            installed_entries: vec![entry_id.clone()],
            compatibility: CompatibilityAssessment {
                game_version: CompatibilityState::Unknown,
                loader: CompatibilityState::Unknown,
                reasons: vec!["restoring previously installed artifact".into()],
            },
            declared_compatibility: record.declared_compatibility,
        }],
        edges: history.previous_dependencies.clone(),
        actions: vec![ResourceEntryAction {
            entry_id: entry_id.clone(),
            node_id,
            kind: ResourceActionKind::Replace,
            target: artifact,
            target_relative_path: history.previous.relative_path.clone(),
            current: Some(current),
        }],
        issues: vec![],
        exceptions: vec![],
        selection_reasons: vec![],
    };
    let draft = plans::OperationDraft {
        local_metadata: Default::default(),
        retained_requirements: Vec::new(),
        resolve_dependencies: true,
        skip_unwritable_files: false,
        import_files: Default::default(),
        naming: Default::default(),
        plan,
        install_dependencies: false,
        candidates: vec![],
        local_files: std::collections::HashMap::from([(
            entry_id,
            history.backup_relative_path.clone().unwrap(),
        )]),
        restore_history: Some(history_id),
    };
    Ok(draft)
}

pub(crate) fn history_entry(
    instance_id: &str,
    current: &InstalledResource,
    target: InstalledResource,
    plan: &ResourceOperationPlan,
    backup_relative_path: Option<String>,
) -> ResourceHistoryEntry {
    let node_id = plan
        .actions
        .iter()
        .find(|action| action.entry_id == current.entry_id)
        .map(|action| &action.node_id);
    let (nodes, edges) = node_id
        .map(|id| super::execution::entry_graph(plan, id))
        .unwrap_or_default();
    ResourceHistoryEntry {
        id: HistoryId::new(uuid::Uuid::new_v4().to_string())
            .expect("UUID is a valid history ID"),
        entry_id: current.entry_id.clone(),
        instance_id: instance_id.to_string(),
        recorded_at: Utc::now(),
        previous: current.clone(),
        target,
        previous_nodes: nodes.clone(),
        target_nodes: nodes,
        previous_dependencies: edges.clone(),
        target_dependencies: edges,
        backup_integrity: Some(current.integrity.clone()),
        backup_available: backup_relative_path.is_some(),
        backup_retired: false,
        backup_relative_path,
    }
}
