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
    check_entry_with_policy(
        service,
        entry,
        target,
        preferred,
        cancel,
        CachePolicy::UseFresh,
    )
    .await
}

pub(crate) async fn check_entry_with_policy(
    service: &std::sync::Arc<ResourceService>,
    entry: &InstalledResource,
    target: &ipc::ResourceTarget,
    preferred: &SourceId,
    cancel: &CancellationToken,
    policy: CachePolicy,
) -> ResourceResult<Option<SourceReleaseRef>> {
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    let Some(artifact) = &entry.artifact else {
        return Ok(None);
    };
    let resource = entry
        .resource_id
        .clone()
        .map(|id| ResourceRef::Resource { id })
        .unwrap_or_else(|| ResourceRef::Source {
            project: artifact.release.project.clone(),
        });
    let current = service
        .release_with_policy(&artifact.release, None, cancel, policy)
        .await?;
    let (_, projects) = service.source_projects(&resource).await?;
    let mut catalog_target = target.clone();
    if matches!(
        entry.resource_type,
        ResourceType::ResourcePack
            | ResourceType::Shader
            | ResourceType::DataPack
            | ResourceType::World
    ) {
        catalog_target.loaders.clear();
    }
    let hash = entry
        .integrity
        .hashes
        .iter()
        .find(|hash| hash.algorithm == HashAlgorithm::Sha1);
    // All confirmed sources participate, even when the installed artifact came
    // from another source. Each source independently falls back only if needed.
    let outcomes = futures::future::join_all(projects.iter().map(|project| {
        project_candidates(
            service,
            project,
            hash,
            &catalog_target,
            target,
            entry.resource_type,
            policy,
            cancel,
        )
    }))
    .await;
    let mut releases = Vec::new();
    let mut failures = Vec::new();
    for (project, outcome) in projects.iter().zip(outcomes) {
        match outcome {
            Ok(candidates) => releases.extend(candidates),
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => {
                failures.push(format!("{}: {error}", project.source.as_str()))
            }
        }
    }
    if !failures.is_empty() {
        return Err(ResourceError::InvalidResponse(format!(
            "Update discovery is incomplete: {}",
            failures.join("; ")
        )));
    }
    releases.retain(|release| {
        release_is_newer(release, &current)
            && release
                .artifacts
                .iter()
                .find(|artifact| artifact.role == ArtifactRole::Primary)
                .is_some_and(|artifact| {
                    !identity::exact_integrity(
                        &entry.integrity,
                        &artifact.integrity,
                    )
                })
    });
    Ok(select_automatic_release(releases, preferred)
        .map(|(release, _)| release.reference))
}

fn usable_candidate(
    release: &ResourceRelease,
    target: &ipc::ResourceTarget,
    kind: ResourceType,
) -> bool {
    let mut primary = release
        .artifacts
        .iter()
        .filter(|artifact| artifact.role == ArtifactRole::Primary);
    let Some(artifact) = primary.next() else {
        return false;
    };
    primary.next().is_none()
        && compatible_candidate(release, target, kind)
        && valid_artifact(artifact)
        && (artifact.integrity.size.is_some()
            || !matches!(
                release.reference.project.source.as_str(),
                "modrinth" | "curseforge"
            ))
        && artifact.available != Some(false)
        && artifact.reference.release == release.reference
        && artifact
            .integrity
            .hashes
            .iter()
            .any(|hash| hash.algorithm != HashAlgorithm::Md5)
}

fn compatible_candidate(
    release: &ResourceRelease,
    target: &ipc::ResourceTarget,
    kind: ResourceType,
) -> bool {
    let compatibility = assess_compatibility(release, target, kind);
    compatibility.game_version == CompatibilityState::Compatible
        && compatibility.loader == CompatibilityState::Compatible
        && (target.channels.is_empty()
            || target.channels.contains(&release.channel))
}

async fn project_candidates(
    service: &std::sync::Arc<ResourceService>,
    project: &SourceProjectRef,
    hash: Option<&ContentHash>,
    catalog_target: &ipc::ResourceTarget,
    target: &ipc::ResourceTarget,
    kind: ResourceType,
    policy: CachePolicy,
    cancel: &CancellationToken,
) -> ResourceResult<Vec<ResourceRelease>> {
    if matches!(project.source.as_str(), "modrinth" | "curseforge")
        && let Some(hash) = hash
    {
        let response = service
            .query(
                &project.source,
                providers::ProviderQuery::UpdateCandidates {
                    project_id: project.project_id.clone(),
                    sha1: hash.value.clone(),
                    target: catalog_target.clone(),
                },
                policy,
                cancel,
            )
            .await;
        match response {
            Ok(QueryOutcome {
                value: providers::ProviderValue::UpdateCandidates(documents),
                ..
            }) if !documents.is_empty()
                && documents.iter().all(|document| {
                    usable_candidate(&document.release, target, kind)
                }) =>
            {
                // These APIs select the latest published compatible files. A
                // current or numeric-only result is still a complete response;
                // compare against installed bytes/version without downloading history.
                tracing::debug!(
                    source = project.source.as_str(),
                    project_id = project.project_id.as_str(),
                    "Resolved update candidates in batch"
                );
                return Ok(documents
                    .into_iter()
                    .map(|document| document.release)
                    .collect());
            }
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            _ => {}
        }
    }
    tracing::debug!(
        source = project.source.as_str(),
        project_id = project.project_id.as_str(),
        "Update candidates require a filtered project catalog"
    );
    let slug = if let Some(store) = service.store() {
        store.project_alias(project).await?
    } else {
        None
    };
    let mut cursor = None;
    let mut seen = std::collections::HashSet::new();
    let mut releases = Vec::new();
    loop {
        let response = service
            .query(
                &project.source,
                providers::ProviderQuery::Releases {
                    project_id: project.project_id.clone(),
                    slug: slug.clone(),
                    target: catalog_target.clone(),
                    cursor: cursor.clone(),
                    page_size: 50,
                },
                policy,
                cancel,
            )
            .await?;
        let providers::ProviderValue::Releases(page) = response.value else {
            return Err(ResourceError::InvalidResponse(
                "source returned no release page".into(),
            ));
        };
        for release in page.entries {
            if !compatible_candidate(&release, target, kind) {
                continue;
            }
            if !usable_candidate(&release, target, kind) {
                return Err(ResourceError::InvalidResponse(
                    "compatible release has invalid primary artifact".into(),
                ));
            }
            releases.push(release);
        }
        if page.window_exhausted {
            return Err(ResourceError::Unsupported(
                "release window exhausted; choose an exact release".into(),
            ));
        }
        cursor = page.cursor;
        match &cursor {
            None => return Ok(releases),
            Some(cursor) if seen.len() < 100 && seen.insert(cursor.clone()) => {
            }
            _ => {
                return Err(ResourceError::InvalidResponse(
                    "release cursor repeated or exceeded its window".into(),
                ));
            }
        }
    }
}
