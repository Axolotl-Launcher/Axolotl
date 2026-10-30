use super::*;
use crate::state::store::{
    content_refresh_checks::{
        CheckAttempt, RefreshCheckContext, RefreshCheckRecord, location_key,
    },
    content_refresh_presentations::PresentationRecord,
};

pub(super) struct EntryWork {
    pub entry: InstalledResource,
    pub file_id: String,
    pub context: RefreshCheckContext,
    pub previous: Option<RefreshCheckRecord>,
    pub content: crate::state::ContentItem,
    pub check_due: bool,
    pub presentation_due: bool,
    pub origin: Option<SourceId>,
}

pub(super) fn work<'a>(
    records: &SnapshotRecords,
    file_ids: impl Iterator<Item = &'a str>,
    mode: RefreshMode,
) -> Vec<EntryWork> {
    let file_ids = file_ids.collect::<HashSet<_>>();
    let entries = records
        .entries
        .iter()
        .filter_map(|entry| Some((entry.file_id.as_deref()?, entry)))
        .collect::<HashMap<_, _>>();
    let checks = records
        .checks
        .iter()
        .map(|(key, check)| (key.entry_id.as_str(), check))
        .collect::<HashMap<_, _>>();
    let target = ipc::ResourceTarget {
        game_versions: vec![records.content_set.game_version.clone()],
        loaders: vec![records.content_set.loader.as_str().into()],
        channels: plans::update_channels(records.instance.update_channel),
    };
    let now = chrono::Utc::now().timestamp();
    records
        .files
        .iter()
        .filter(|file| !file.missing && file_ids.contains(file.id.as_str()))
        .filter_map(|file| {
            let entry = entries.get(file.id.as_str())?;
            let kind = match file.relative_path.split('/').next()? {
                "mods" => ResourceType::Mod,
                "plugins" => ResourceType::Plugin,
                "resourcepacks" => ResourceType::ResourcePack,
                "shaderpacks" => ResourceType::Shader,
                "saves" if file.relative_path.contains("/datapacks/") => {
                    ResourceType::DataPack
                }
                _ => return None,
            };
            let integrity = ArtifactIntegrity {
                size: Some(file.size),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: file.sha1.clone(),
                }],
            };
            let artifact = records.artifacts.get(&entry.id).filter(|record| {
                super::super::valid_artifact(&record.artifact)
                    && identity::exact_integrity(
                        &integrity,
                        &record.artifact.integrity,
                    )
            });
            let context = RefreshCheckContext {
                location_key: location_key(&records.instance),
                content_root: records
                    .fingerprints
                    .get(&file.id)
                    .map(|record| record.content_root.clone())
                    .unwrap_or_default(),
                sha1: file.sha1.clone(),
                size: file.size,
                current_release: artifact
                    .map(|record| record.artifact.reference.release.clone()),
                target: target.clone(),
                preferred_source: records.preferred_source.clone(),
            };
            let previous = checks
                .get(entry.id.as_str())
                .filter(|check| {
                    check.file_id == file.id
                        && check.context.location_key == context.location_key
                        && check.context.content_root == context.content_root
                        && check.context.sha1 == context.sha1
                        && check.context.size == context.size
                        && check.context.current_release
                            == context.current_release
                        && check.context.target == context.target
                        && check.context.preferred_source
                            == context.preferred_source
                })
                .map(|check| (*check).clone());
            if mode == RefreshMode::RetryFailed
                && previous
                    .as_ref()
                    .is_none_or(|check| check.attempt != CheckAttempt::Failed)
            {
                return None;
            }
            let check_due = if mode == RefreshMode::RetryFailed {
                previous
                    .as_ref()
                    .is_none_or(|check| check.attempt == CheckAttempt::Failed)
            } else {
                due(previous.as_ref(), now, mode)
            };
            let presentation_due = mode == RefreshMode::ForceRemote
                || (check_due && mode == RefreshMode::RetryFailed)
                || records.presentations.get(&file.id).map_or_else(
                    || {
                        previous.as_ref().is_none_or(|check| {
                            check.attempt == CheckAttempt::Succeeded
                                || now.saturating_sub(check.attempted_at)
                                    >= REMOTE_CACHE_SECONDS
                        })
                    },
                    |record| {
                        record.location_key != context.location_key
                            || record.content.id != context.sha1
                            || (now.saturating_sub(record.saved_at)
                                >= REMOTE_CACHE_SECONDS
                                && previous.as_ref().is_none_or(|check| {
                                    check.attempt != CheckAttempt::Failed
                                        || now
                                            .saturating_sub(check.attempted_at)
                                            >= REMOTE_CACHE_SECONDS
                                }))
                    },
                );
            if !check_due && !presentation_due {
                return None;
            }
            let scope = if kind == ResourceType::DataPack
                && file.relative_path.starts_with("saves/")
            {
                InstallScope::World {
                    relative_path: file
                        .relative_path
                        .split('/')
                        .take(2)
                        .collect::<Vec<_>>()
                        .join("/"),
                }
            } else {
                InstallScope::Instance
            };
            let ownership = match entry.ownership_kind {
                crate::state::ContentOwnershipKind::PackManaged => {
                    ResourceOwnership::PackManaged {
                        pack_member_id: records
                            .members
                            .iter()
                            .find(|member| {
                                member.content_entry_id.as_ref()
                                    == Some(&entry.id)
                            })?
                            .id
                            .clone(),
                    }
                }
                crate::state::ContentOwnershipKind::UserAdded => {
                    ResourceOwnership::UserAdded
                }
                _ => ResourceOwnership::LocalDiscovered,
            };
            let refs = records
                .references
                .get(&entry.id)
                .cloned()
                .unwrap_or_default();
            let content = crate::state::ContentItem {
                file_name: file.file_name.clone(),
                file_path: file.relative_path.clone(),
                id: file.sha1.clone(),
                size: file.size,
                enabled: file.enabled && entry.enabled,
                project_type: entry.project_type,
                project: None,
                version: None,
                owner: None,
                update: None,
                date_added: Some(file.added_at.to_rfc3339()),
                provider_refs: refs,
                origin_provider: records.origins.get(&entry.id).copied(),
                rollback: None,
                environment: None,
                source_kind: Some(entry.source_kind),
                external: artifact.is_none(),
                loader: None,
            };
            Some(EntryWork {
                entry: InstalledResource {
                    entry_id: InstalledEntryId::new(entry.id.clone()).ok()?,
                    instance_id: records.instance.id.clone(),
                    resource_id: records
                        .entry_resources
                        .get(&entry.id)
                        .cloned(),
                    artifact: artifact
                        .map(|record| record.artifact.reference.clone()),
                    known_artifacts: artifact
                        .map(|record| vec![record.artifact.reference.clone()])
                        .unwrap_or_default(),
                    integrity,
                    relative_path: file.relative_path.clone(),
                    original_file_name: artifact
                        .and_then(|record| record.artifact.file_name.clone()),
                    resource_type: kind,
                    scope,
                    enabled: file.enabled,
                    missing: false,
                    ownership,
                },
                file_id: file.id.clone(),
                context,
                previous,
                content,
                check_due,
                presentation_due,
                origin: records
                    .origins
                    .get(&entry.id)
                    .and_then(|origin| SourceId::new(origin.as_str()).ok()),
            })
        })
        .collect()
}

fn due(
    previous: Option<&RefreshCheckRecord>,
    now: i64,
    mode: RefreshMode,
) -> bool {
    if mode != RefreshMode::Normal {
        return true;
    }
    match previous {
        Some(check) if check.attempt == CheckAttempt::Succeeded => {
            check.last_success.as_ref().is_none_or(|success| {
                now.saturating_sub(success.checked_at) >= REMOTE_CACHE_SECONDS
            })
        }
        Some(check) => {
            now.saturating_sub(check.attempted_at) >= REMOTE_CACHE_SECONDS
        }
        None => true,
    }
}

pub(super) async fn run(
    state: &crate::State,
    service: Arc<ResourceService>,
    mut work: EntryWork,
    policy: CachePolicy,
    cancel: &CancellationToken,
    tx: mpsc::Sender<RefreshWrite>,
) -> ResourceResult<()> {
    let instance = crate::state::store::instance_content_files::get_instance(
        &state.data,
        &work.entry.instance_id,
    )
    .await?
    .ok_or_else(|| {
        ResourceError::InvalidRequest("Instance was deleted".into())
    })?;
    if location_key(&instance) != work.context.location_key {
        return Err(ResourceError::Canceled);
    }
    let root = crate::state::instances::commands::instance_content_root(
        &state.directories,
        &instance,
    )?;
    work.context.content_root = root.to_string_lossy().into_owned();
    let mut artifact_write = None;
    if work.entry.artifact.is_none() {
        let recognition =
            recognize(state, &service, &root, &mut work, policy, cancel).await;
        match recognition {
            Ok(artifact) => artifact_write = artifact,
            Err(error) => {
                let mut check = record(&work);
                check.failed(chrono::Utc::now().timestamp(), error.to_string());
                send(&tx, &work, Some(check), None, None).await?;
                return Ok(());
            }
        }
    }
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if let Some(record) = artifact_write {
        send(&tx, &work, None, None, Some(record.clone())).await?;
        work.context.current_release = Some(record.artifact.reference.release);
    }
    let Some(reference) = work.entry.artifact.as_ref() else {
        let mut check = record(&work);
        check.attempt = CheckAttempt::Unrecognized;
        check.attempted_at = chrono::Utc::now().timestamp();
        check.error = None;
        send(&tx, &work, Some(check), None, None).await?;
        return Ok(());
    };
    let update = async {
        if !work.check_due {
            return Ok(None);
        }
        let outcome = updates::check_entry_with_policy(
            &service,
            &work.entry,
            &work.context.target,
            &work.context.preferred_source,
            cancel,
            policy,
        )
        .await;
        let mut check = record(&work);
        match outcome {
            Ok(candidate) => {
                check.succeeded(chrono::Utc::now().timestamp(), candidate)
            }
            Err(ResourceError::NoCompatibleRelease) => {
                check.succeeded(chrono::Utc::now().timestamp(), None)
            }
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => {
                check.failed(chrono::Utc::now().timestamp(), error.to_string())
            }
        }
        send(&tx, &work, Some(check.clone()), None, None).await?;
        Ok::<_, ResourceError>(Some(check))
    };
    let presentation = async {
        if !work.presentation_due {
            return Ok(());
        }
        let project = async {
            let document = service
                .project_document_with_policy(
                    &reference.release.project,
                    None,
                    cancel,
                    policy,
                )
                .await?;
            let project = document.metadata;
            let mut content = work.content.clone();
            content.project = Some(crate::state::ContentItemProject {
                id: project.project.project_id.as_str().into(),
                slug: project.slug,
                title: project.title,
                icon_url: project.icon_url,
                license: None,
            });
            content.owner = document
                .owner
                .map(|owner| crate::state::ContentItemOwner {
                    id: owner.id,
                    name: owner.name,
                    avatar_url: owner.avatar_url,
                    owner_type: if owner.organization {
                        crate::state::OwnerType::Organization
                    } else {
                        crate::state::OwnerType::User
                    },
                })
                .or_else(|| {
                    project.authors.first().map(|author| {
                        crate::state::ContentItemOwner {
                            id: if project.project.source
                                == SourceId::modrinth()
                            {
                                author.clone()
                            } else {
                                format!("local:{author}")
                            },
                            name: author.clone(),
                            avatar_url: None,
                            owner_type: crate::state::OwnerType::User,
                        }
                    })
                });
            content.external = false;
            send(
                &tx,
                &work,
                None,
                Some(PresentationRecord {
                    location_key: work.context.location_key.clone(),
                    saved_at: chrono::Utc::now().timestamp(),
                    content,
                }),
                None,
            )
            .await
        };
        let release = async {
            let release = service
                .release_with_policy(&reference.release, None, cancel, policy)
                .await?;
            let mut content = work.content.clone();
            content.version = Some(crate::state::ContentItemVersion {
                id: release.reference.release_id.as_str().into(),
                version_number: release.version,
                file_name: content.file_name.clone(),
                date_published: release
                    .published_at
                    .map(|date| date.to_rfc3339()),
            });
            content.loader = release.compatibility.loaders.first().cloned();
            send(
                &tx,
                &work,
                None,
                Some(PresentationRecord {
                    location_key: work.context.location_key.clone(),
                    saved_at: chrono::Utc::now().timestamp(),
                    content,
                }),
                None,
            )
            .await
        };
        let (project, release) = tokio::join!(project, release);
        project?;
        release
    };
    let (update, presentation) = tokio::join!(update, presentation);
    let check = update?;
    if let Err(error) = presentation {
        if error == ResourceError::Canceled {
            return Err(error);
        }
        let mut check = check.unwrap_or_else(|| record(&work));
        check.failed(chrono::Utc::now().timestamp(), error.to_string());
        send(&tx, &work, Some(check), None, None).await?;
    }
    Ok(())
}

fn record(work: &EntryWork) -> RefreshCheckRecord {
    RefreshCheckRecord::for_context(
        work.previous.clone(),
        work.file_id.clone(),
        work.context.clone(),
        chrono::Utc::now().timestamp(),
    )
}

pub(super) async fn bytes_still_current(
    state: &crate::State,
    instance_id: &str,
    write: &RefreshWrite,
) -> ResourceResult<bool> {
    use crate::state::store::{
        Collection as _,
        content_file_fingerprints::{
            ContentFileFingerprints, FileModifiedTime, FingerprintKey,
        },
    };
    let Some(record) = ContentFileFingerprints::get(
        &state.data,
        &FingerprintKey {
            instance_id: instance_id.into(),
            file_id: write.file_id.clone(),
        },
    )
    .await?
    else {
        return Ok(false);
    };
    if record.content_root != write.context.content_root
        || record.sha1() != write.context.sha1
        || record.size != write.context.size
    {
        return Ok(false);
    }
    let path = super::super::execution::checked_path(
        std::path::Path::new(&record.content_root),
        &record.relative_path,
    )?;
    let Ok(metadata) = tokio::fs::metadata(path).await else {
        return Ok(false);
    };
    Ok(record.matches(
        &write.context.content_root,
        &record.relative_path,
        metadata.len(),
        metadata
            .modified()
            .ok()
            .and_then(FileModifiedTime::from_system_time)
            .as_ref(),
    ))
}

async fn send(
    tx: &mpsc::Sender<RefreshWrite>,
    work: &EntryWork,
    check: Option<RefreshCheckRecord>,
    presentation: Option<PresentationRecord>,
    artifact: Option<crate::resources::installed::InstalledArtifactRecord>,
) -> ResourceResult<()> {
    tx.send(RefreshWrite {
        entry_id: work.entry.entry_id.as_str().into(),
        file_id: work.file_id.clone(),
        context: work.context.clone(),
        check,
        presentation,
        artifact,
    })
    .await
    .map_err(|_| ResourceError::Canceled)
}

async fn recognize(
    state: &crate::State,
    service: &Arc<ResourceService>,
    root: &std::path::Path,
    work: &mut EntryWork,
    policy: CachePolicy,
    cancel: &CancellationToken,
) -> ResourceResult<Option<crate::resources::installed::InstalledArtifactRecord>>
{
    let local = installed::installed_file_identity(
        state,
        &work.entry.instance_id,
        root,
        &work.entry,
        service.cache(),
        true,
        cancel,
    )
    .await?;
    let sources = service
        .sources()
        .into_iter()
        .filter(|source| {
            source.available
                && matches!(source.id.as_str(), "modrinth" | "curseforge")
        })
        .map(|source| source.id)
        .collect::<Vec<_>>();
    let mut recognition = identity::FileRecognition {
        matches: vec![],
        unresolved_sources: vec![],
        failures: vec![],
        ambiguous: false,
    };
    for reference in &work.content.provider_refs {
        let Some(release_id) = reference.database_release_id() else {
            continue;
        };
        let reference = SourceReleaseRef {
            project: SourceProjectRef {
                source: SourceId::new(reference.provider().as_str())?,
                project_id: SourceProjectId::new(
                    reference.database_project_id(),
                )?,
            },
            release_id: SourceReleaseId::new(release_id)?,
        };
        if let Ok(release) = service
            .release_with_policy(&reference, None, cancel, policy)
            .await
        {
            recognition
                .matches
                .extend(release.artifacts.into_iter().filter(|artifact| {
                    artifact.role == ArtifactRole::Primary
                        && identity::exact_integrity(
                            &local.integrity,
                            &artifact.integrity,
                        )
                }));
        }
    }
    let origin_verified = work.origin.as_ref().is_some_and(|origin| {
        recognition.matches.iter().any(|artifact| {
            &artifact.reference.release.project.source == origin
        })
    });
    if recognition.matches.is_empty()
        || (work.origin.is_some() && !origin_verified)
    {
        let known = recognition.matches;
        recognition = service
            .identify_many(
                &sources,
                std::slice::from_ref(&local),
                policy,
                cancel,
            )
            .await
            .into_iter()
            .next()
            .ok_or(ResourceError::Unavailable)?;
        for artifact in known {
            if !recognition
                .matches
                .iter()
                .any(|found| found.reference == artifact.reference)
            {
                recognition.matches.push(artifact);
            }
        }
    }
    let mut projects = HashMap::<SourceId, HashSet<SourceProjectId>>::new();
    for artifact in &recognition.matches {
        projects
            .entry(artifact.reference.release.project.source.clone())
            .or_default()
            .insert(artifact.reference.release.project.project_id.clone());
    }
    recognition.ambiguous |=
        projects.values().any(|projects| projects.len() > 1);
    let selected = service
        .select_recognized_artifact(
            &recognition.matches,
            work.entry.artifact.as_ref(),
            work.origin.as_ref(),
            &work.context.preferred_source,
            work.entry.resource_type,
            recognition.ambiguous,
            policy,
            cancel,
        )
        .await?;
    if selected.is_none() && !recognition.failures.is_empty() {
        return Err(recognition.failures[0].1.clone());
    }
    let Some((artifact, project)) = selected else {
        return Ok(None);
    };
    if let Some(store) = service.store() {
        work.entry.resource_id = Some(
            store
                .ensure_project(&project.project, &project.types)
                .await?,
        );
        for matched in &recognition.matches {
            if matched.reference.release.project == project.project {
                continue;
            }
            if !identity::exact_integrity(
                &artifact.integrity,
                &matched.integrity,
            ) || recognition.ambiguous
            {
                continue;
            }
            if store.lookup(&matched.reference.release.project).await?
                == work.entry.resource_id
            {
                continue;
            }
            let other = match service
                .project_document_with_policy(
                    &matched.reference.release.project,
                    None,
                    cancel,
                    policy,
                )
                .await
            {
                Ok(other) => other,
                Err(
                    error @ (ResourceError::Canceled
                    | ResourceError::Database(_)),
                ) => return Err(error),
                Err(_) => continue,
            };
            identity::try_confirm_projects(
                store,
                &project,
                &other.metadata,
                &artifact,
                matched,
                cancel,
            )
            .await?;
        }
    }
    work.entry.artifact = Some(artifact.reference.clone());
    work.entry.known_artifacts = vec![artifact.reference.clone()];
    let declaration = service
        .release_with_policy(&artifact.reference.release, None, cancel, policy)
        .await
        .ok()
        .map(|release| release.compatibility);
    Ok(Some(crate::resources::installed::InstalledArtifactRecord {
        artifact,
        declared_compatibility: declaration,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_checks_reuse_a_day_and_explicit_refresh_bypasses_it() {
        let mut check: RefreshCheckRecord = serde_json::from_value(serde_json::json!({
            "file_id": "file", "context": { "location_key": "location", "content_root": "root", "sha1": "a".repeat(40), "size": 1, "current_release": null,
                "target": { "gameVersions": ["1.21"], "loaders": ["fabric"], "channels": [] }, "preferred_source": "modrinth" },
            "last_success": null, "attempted_at": 100, "attempt": "unrecognized", "error": null
        })).unwrap();
        check.succeeded(100, None);
        assert!(!due(
            Some(&check),
            100 + REMOTE_CACHE_SECONDS - 1,
            RefreshMode::Normal
        ));
        assert!(due(
            Some(&check),
            100 + REMOTE_CACHE_SECONDS,
            RefreshMode::Normal
        ));
        check.failed(200, "offline".into());
        // A failed source response must not restart on each page visit or tick.
        for seconds in [0, 1, 2, 30, 60] {
            assert!(!due(Some(&check), 200 + seconds, RefreshMode::Normal));
        }
        assert!(!due(
            Some(&check),
            200 + REMOTE_CACHE_SECONDS - 1,
            RefreshMode::Normal
        ));
        assert!(due(
            Some(&check),
            200 + REMOTE_CACHE_SECONDS,
            RefreshMode::Normal
        ));
        assert!(due(Some(&check), 200, RefreshMode::ForceRemote));
        assert!(due(Some(&check), 200, RefreshMode::RetryFailed));
    }
}
