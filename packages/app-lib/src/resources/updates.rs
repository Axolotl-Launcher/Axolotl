use super::*;
use futures::{StreamExt, stream};
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
    let all_ids;
    let ids = match entry_ids {
        Some(ids) => ids,
        None => {
            all_ids = plans::installed(&state.pool, &instance_id)
                .await?
                .into_iter()
                .map(|entry| entry.entry_id)
                .collect::<Vec<_>>();
            &all_ids
        }
    };
    // Discover against the displayed content set. A full planning hydration
    // would rescan files and resolve retained dependencies before any result.
    let hydrated = installed::hydrate_update_batch_with_service(
        &state,
        &instance_id,
        cancel,
        ids,
        service,
    )
    .await?;
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
    let result = ResourceUpdateCheck {
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
    // Discovery reports newer compatible releases. Dependency graph and exact
    // candidate JAR validation belong to the reviewed apply plan, where a
    // failure can be handled without suppressing every update in the instance.
    check_entries(
        service,
        hydrated.entries,
        result,
        &target,
        &preferred,
        on_progress,
        cancel,
    )
    .await
}

pub(crate) async fn check_entries(
    service: &std::sync::Arc<ResourceService>,
    entries: Vec<InstalledResource>,
    mut result: ResourceUpdateCheck,
    target: &ipc::ResourceTarget,
    preferred: &SourceId,
    on_progress: impl Fn(ResourceUpdateProgress) + Send + Sync,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceUpdateCheck> {
    let failed = &result.failed_entries;
    let entries = entries
        .into_iter()
        .filter(|entry| !entry.missing && !failed.contains(&entry.entry_id))
        .collect::<Vec<_>>();
    let mut checks = stream::iter(entries)
        .map(|entry| async move {
            let outcome =
                check_entry(service, &entry, target, preferred, cancel).await;
            (entry, outcome)
        })
        .buffer_unordered(8);
    while let Some((entry, outcome)) = checks.next().await {
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let unresolved = entry.artifact.is_none();
        match outcome {
            Ok(Some(release)) => {
                result.releases.insert(entry.entry_id.clone(), release);
                result.entries.push(entry.entry_id.clone());
            }
            Ok(None) => {
                if unresolved {
                    result.unresolved.push(entry.relative_path.clone());
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
        on_progress(result.progress_for(&entry.entry_id, unresolved));
    }
    Ok(result)
}

async fn check_entry(
    service: &std::sync::Arc<ResourceService>,
    entry: &InstalledResource,
    target: &ipc::ResourceTarget,
    preferred: &SourceId,
    cancel: &CancellationToken,
) -> ResourceResult<Option<SourceReleaseRef>> {
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    let Some(artifact) = &entry.artifact else {
        return Ok(None);
    };
    let selection = ResourceSelection {
        id: SelectionId::new(entry.entry_id.as_str())?,
        resource: entry
            .resource_id
            .clone()
            .map(|id| ResourceRef::Resource { id })
            .unwrap_or_else(|| ResourceRef::Source {
                project: artifact.release.project.clone(),
            }),
        release: ReleaseSelection::Automatic,
        resource_type: entry.resource_type,
        scope: entry.scope.clone(),
        entry_id: Some(entry.entry_id.clone()),
    };
    let current = service.release(&artifact.release, None, cancel).await?;
    let (release, artifact, _) = service
        .selected_release_for_current(
            &selection,
            target,
            preferred,
            cancel,
            Some(&current),
        )
        .await?;
    Ok(
        (!identity::exact_integrity(&entry.integrity, &artifact.integrity)
            && release_is_newer(&release, &current))
        .then_some(release.reference),
    )
}
