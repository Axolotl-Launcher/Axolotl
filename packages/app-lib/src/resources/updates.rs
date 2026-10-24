use super::*;
use tokio_util::sync::CancellationToken;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdateCheck {
    pub entries: Vec<InstalledEntryId>,
    pub releases: std::collections::HashMap<InstalledEntryId, SourceReleaseRef>,
    pub unresolved: Vec<String>,
    pub failures: Vec<String>,
    pub failed_entries: Vec<InstalledEntryId>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUpdateProgress {
    pub entry_id: InstalledEntryId,
    pub release: Option<SourceReleaseRef>,
    pub failed: bool,
    pub unresolved: bool,
}

impl ResourceUpdateCheck {
    fn progress_for(
        &self,
        entry_id: &InstalledEntryId,
        unresolved: bool,
    ) -> ResourceUpdateProgress {
        ResourceUpdateProgress {
            entry_id: entry_id.clone(),
            release: self.releases.get(entry_id).cloned(),
            failed: self.failed_entries.contains(entry_id),
            unresolved,
        }
    }
}

pub async fn check(
    instance_id: String,
    entry_ids: Option<&[InstalledEntryId]>,
    on_progress: impl Fn(ResourceUpdateProgress) + Send + Sync,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceUpdateCheck> {
    let state = crate::State::get().await?;
    let service = state.resources().await?;
    let hydrated = match entry_ids {
        Some(ids) => {
            installed::hydrate_update_batch_with_service(
                &state,
                &instance_id,
                cancel,
                ids,
                service,
            )
            .await?
        }
        None => {
            installed::hydrate_with_failures(&state, &instance_id, cancel)
                .await?
        }
    };
    let instance = crate::state::get_instance(&instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            ResourceError::InvalidRequest("unknown instance".into())
        })?;
    let target = ipc::ResourceTarget {
        game_versions: vec![instance.applied_content_set.game_version],
        loaders: vec![instance.applied_content_set.loader.as_str().into()],
        channels: plans::update_channels(instance.instance.update_channel),
    };
    let preferred = service.preferred_source().await?;
    let mut result = ResourceUpdateCheck {
        entries: vec![],
        releases: std::collections::HashMap::new(),
        unresolved: vec![],
        failures: hydrated
            .failures
            .iter()
            .map(|(_, error)| error.clone())
            .collect(),
        failed_entries: hydrated
            .failures
            .iter()
            .map(|(id, _)| id.clone())
            .collect(),
    };
    for entry_id in &result.failed_entries {
        on_progress(result.progress_for(entry_id, false));
    }
    // Visible-entry discovery must not hydrate every retained parent. The
    // operation planner verifies the complete dependency graph before applying
    // an update; explicit full-instance checks also retain that validation here.
    let requirements = if entry_ids.is_none() {
        let local =
            plans::requirements::local_metadata(&state.pool, &instance_id)
                .await?;
        service
            .retained_requirements(&[], &hydrated.entries, &local, cancel)
            .await?
    } else {
        Vec::new()
    };
    for entry in hydrated.entries.into_iter().filter(|entry| !entry.missing) {
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        if result.failed_entries.contains(&entry.entry_id) {
            continue;
        }
        let Some(artifact) = &entry.artifact else {
            result.unresolved.push(entry.relative_path);
            on_progress(result.progress_for(&entry.entry_id, true));
            continue;
        };
        let selection = ResourceSelection {
            id: SelectionId::new(entry.entry_id.as_str())?,
            resource: entry
                .resource_id
                .map(|id| ResourceRef::Resource { id })
                .unwrap_or_else(|| ResourceRef::Source {
                    project: artifact.release.project.clone(),
                }),
            release: ReleaseSelection::Automatic,
            resource_type: entry.resource_type,
            scope: entry.scope,
            entry_id: Some(entry.entry_id.clone()),
        };
        let current =
            match service.release(&artifact.release, None, cancel).await {
                Ok(current) => current,
                Err(ResourceError::Canceled) => {
                    return Err(ResourceError::Canceled);
                }
                Err(error) => {
                    result.failed_entries.push(entry.entry_id.clone());
                    result
                        .failures
                        .push(format!("{}: {error}", entry.relative_path));
                    on_progress(result.progress_for(&entry.entry_id, false));
                    continue;
                }
            };
        let retained =
            service.requirements_for(&selection, &requirements).await?;
        match service
            .selected_release_with_requirements(
                &selection,
                &target,
                &preferred,
                cancel,
                Some(&current),
                &[],
                &retained,
            )
            .await
        {
            Ok((release, target, _)) => {
                if identity::exact_integrity(
                    &entry.integrity,
                    &target.integrity,
                ) {
                    on_progress(result.progress_for(&entry.entry_id, false));
                    continue;
                }
                if release_is_newer(&release, &current) {
                    result
                        .releases
                        .insert(entry.entry_id.clone(), release.reference);
                    result.entries.push(entry.entry_id.clone())
                }
            }
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(ResourceError::NoCompatibleRelease) => {}
            Err(error) => {
                result.failed_entries.push(entry.entry_id.clone());
                result
                    .failures
                    .push(format!("{}: {error}", entry.relative_path));
            }
        }
        on_progress(result.progress_for(&entry.entry_id, false));
    }
    Ok(result)
}
