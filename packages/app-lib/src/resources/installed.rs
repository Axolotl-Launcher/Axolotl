use super::{identity::LocalArtifactIdentity, *};
use tokio_util::sync::CancellationToken;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledArtifactRecord {
    #[serde(flatten)]
    pub artifact: ResourceArtifact,
    #[serde(default)]
    pub declared_compatibility: Option<ReleaseCompatibility>,
}

pub(crate) async fn attach_compatibility(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
    game_version: &str,
    loader: &str,
    items: &mut [crate::state::instances::InstanceContentSnapshotItem],
    service: Option<&std::sync::Arc<ResourceService>>,
) -> crate::Result<()> {
    let rows: Vec<(String, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT entry.id, file.sha1, file.size, file.local_mod_data, exact.artifact FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id LEFT JOIN resource_installed_artifacts exact ON exact.entry_id = entry.id WHERE entry.instance_id = ? AND file.missing = 0"
    ).bind(instance_id).fetch_all(pool).await?;
    let target = ipc::ResourceTarget {
        game_versions: vec![game_version.into()],
        loaders: vec![loader.into()],
        channels: vec![],
    };
    let indices: std::collections::HashMap<_, _> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.entry_id.clone().map(|id| (id, index)))
        .collect();
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(id, sha1, size, local, artifact)| {
            let record = artifact.and_then(|payload| {
                serde_json::from_str::<InstalledArtifactRecord>(&payload).ok()
            });
            (id, sha1, size, local, record)
        })
        .collect();
    let mut cached = vec![None; rows.len()];
    if let Some(service) = service {
        let mut requested = Vec::new();
        let files = rows
            .iter()
            .enumerate()
            .filter_map(|(index, (id, sha1, size, _, record))| {
                let item =
                    indices.get(id).and_then(|index| items.get(*index))?;
                let integrity = ArtifactIntegrity {
                    size: u64::try_from(*size).ok(),
                    hashes: vec![ContentHash {
                        algorithm: HashAlgorithm::Sha1,
                        value: sha1.clone(),
                    }],
                };
                if record
                    .as_ref()
                    .filter(|record| {
                        identity::exact_integrity(
                            &integrity,
                            &record.artifact.integrity,
                        )
                    })
                    .and_then(|record| record.declared_compatibility.as_ref())
                    .is_some_and(|declaration| {
                        !declaration.game_versions.is_empty()
                            && !declaration.loaders.is_empty()
                    })
                {
                    return None;
                }
                if item.project_type != crate::state::ProjectType::Mod {
                    return None;
                }
                requested.push(index);
                let mut references = record
                    .as_ref()
                    .map(|record| record.artifact.reference.release.clone())
                    .into_iter()
                    .collect::<Vec<_>>();
                if let Some(item) =
                    indices.get(id).and_then(|index| items.get(*index))
                {
                    for reference in item
                        .content
                        .iter()
                        .flat_map(|content| &content.provider_refs)
                    {
                        let Some(release_id) = reference.database_release_id()
                        else {
                            continue;
                        };
                        if let (Ok(source), Ok(project_id), Ok(release_id)) = (
                            SourceId::new(reference.provider().as_str()),
                            SourceProjectId::new(
                                reference.database_project_id(),
                            ),
                            SourceReleaseId::new(release_id),
                        ) {
                            references.push(SourceReleaseRef {
                                project: SourceProjectRef {
                                    source,
                                    project_id,
                                },
                                release_id,
                            });
                        }
                    }
                }
                Some((integrity, references))
            })
            .collect::<Vec<_>>();
        for (index, declaration) in requested
            .into_iter()
            .zip(cached_file_compatibility(service, &files).await)
        {
            cached[index] = declaration;
        }
    }
    let projects = rows
        .iter()
        .filter_map(|(id, sha1, size, _, artifact)| {
            let item = items.get(*indices.get(id)?)?;
            if item.resource_id.is_some() {
                return None;
            }
            artifact
                .as_ref()
                .filter(|record| {
                    super::identity::exact_integrity(
                        &ArtifactIntegrity {
                            size: u64::try_from(*size).ok(),
                            hashes: vec![ContentHash {
                                algorithm: HashAlgorithm::Sha1,
                                value: sha1.clone(),
                            }],
                        },
                        &record.artifact.integrity,
                    )
                })
                .map(|record| record.artifact.reference.release.project.clone())
        })
        .collect::<Vec<_>>();
    let identities = crate::state::resources::ResourceStore::new(pool.clone())
        .lookup_many(&projects)
        .await
        .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?;
    for ((id, sha1, size, local, artifact), cached) in
        rows.into_iter().zip(cached)
    {
        let Some(&index) = indices.get(&id) else {
            continue;
        };
        let item = &mut items[index];
        if let Some(record) = artifact.as_ref() {
            let local = ArtifactIntegrity {
                size: u64::try_from(size).ok(),
                hashes: vec![ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: sha1.clone(),
                }],
            };
            if super::identity::exact_integrity(
                &local,
                &record.artifact.integrity,
            ) {
                item.artifact_ref = Some(record.artifact.reference.clone());
                if !record.artifact.integrity.hashes.is_empty()
                    && item.content.is_some()
                    && item.materialization_state
                        == crate::state::PackMemberMaterializationState::Present
                {
                    item.capabilities.can_change_version = true;
                }
                if item.resource_id.is_none() {
                    item.resource_id = identities
                        .get(&record.artifact.reference.release.project)
                        .cloned();
                }
            }
        }
        let kind = match item.project_type {
            crate::state::ProjectType::Mod => ResourceType::Mod,
            crate::state::ProjectType::ResourcePack => {
                ResourceType::ResourcePack
            }
            crate::state::ProjectType::ShaderPack => ResourceType::Shader,
            crate::state::ProjectType::DataPack => ResourceType::DataPack,
            _ => continue,
        };
        item.compatibility = None;
        let exact = artifact.filter(|artifact| {
            artifact.artifact.integrity.size == u64::try_from(size).ok()
                && artifact.artifact.integrity.hashes.iter().any(|hash| {
                    hash.algorithm == HashAlgorithm::Sha1
                        && hash.value.eq_ignore_ascii_case(&sha1)
                })
        });
        let local_declared = (|| {
            let metadata = serde_json::from_str::<
                crate::mod_metadata::LocalModMetadata,
            >(local.as_deref()?)
            .ok()?;
            Some(ReleaseCompatibility {
                game_versions: metadata.minecraft_version.into_iter().collect(),
                loaders: metadata.loader.into_iter().collect(),
            })
        })();
        let mut remote = exact.and_then(|record| record.declared_compatibility);
        if let Some(cached) = cached {
            let remote = remote.get_or_insert_with(|| cached.clone());
            if remote.game_versions.is_empty() {
                remote.game_versions = cached.game_versions;
            }
            if remote.loaders.is_empty() {
                remote.loaders = cached.loaders;
            }
        }
        let declared = match (remote, local_declared) {
            (Some(mut remote), Some(local)) => {
                if remote.game_versions.is_empty() {
                    remote.game_versions = local.game_versions;
                }
                if remote.loaders.is_empty() {
                    remote.loaders = local.loaders;
                }
                Some(remote)
            }
            (remote, local) => remote.or(local),
        };
        if let Some(declared) = declared {
            item.compatibility =
                Some(assess_declared_compatibility(&declared, &target, kind));
        }
    }
    Ok(())
}

pub(crate) async fn cached_file_compatibility(
    service: &std::sync::Arc<ResourceService>,
    files: &[(ArtifactIntegrity, Vec<SourceReleaseRef>)],
) -> Vec<Option<ReleaseCompatibility>> {
    use super::providers::{ProviderQuery, ProviderValue};
    let mut declarations = vec![None; files.len()];
    if files.is_empty() {
        return declarations;
    }
    let cancel = CancellationToken::new();
    let mut sources = service.sources();
    let preferred = service
        .preferred_source()
        .await
        .unwrap_or_else(|_| SourceId::modrinth());
    sources.sort_by_key(|source| {
        (
            source.id != preferred,
            source.id != SourceId::modrinth(),
            source.id.clone(),
        )
    });
    for source in sources {
        let mut queries = Vec::new();
        let mut indices = Vec::new();
        for (index, (integrity, references)) in files.iter().enumerate() {
            if source.id == SourceId::modrinth()
                && let Some(hash) = integrity
                    .hashes
                    .iter()
                    .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
            {
                queries.push(ProviderQuery::IdentifyExport {
                    integrity: ArtifactIntegrity {
                        size: None,
                        hashes: vec![ContentHash {
                            algorithm: HashAlgorithm::Sha1,
                            value: hash.value.to_ascii_lowercase(),
                        }],
                    },
                    fingerprint: None,
                });
                indices.push(index);
            }
            for reference in references
                .iter()
                .filter(|reference| reference.project.source == source.id)
            {
                queries.push(ProviderQuery::Release {
                    project_id: Some(reference.project.project_id.clone()),
                    release_id: reference.release_id.clone(),
                    slug: None,
                });
                indices.push(index);
            }
        }
        let mut matches =
            std::collections::HashMap::<usize, Vec<ResourceRelease>>::new();
        for (index, result) in indices.into_iter().zip(
            service
                .query_many(
                    &source.id,
                    queries,
                    CachePolicy::OnlyCached,
                    &cancel,
                )
                .await,
        ) {
            let Ok(result) = result else { continue };
            let documents = match result.value {
                ProviderValue::Identified(documents) => documents,
                ProviderValue::Release(document) => vec![document],
                _ => continue,
            };
            for document in documents {
                let release = document.release;
                if release.artifacts.iter().any(|artifact| {
                    artifact.role == ArtifactRole::Primary
                        && identity::exact_integrity(
                            &files[index].0,
                            &artifact.integrity,
                        )
                }) {
                    let values = matches.entry(index).or_default();
                    if !values
                        .iter()
                        .any(|value| value.reference == release.reference)
                    {
                        values.push(release);
                    }
                }
            }
        }
        for (index, releases) in matches {
            if let [release] = releases.as_slice() {
                let declaration = declarations[index]
                    .get_or_insert_with(|| release.compatibility.clone());
                if declaration.game_versions.is_empty() {
                    declaration.game_versions =
                        release.compatibility.game_versions.clone();
                }
                if declaration.loaders.is_empty() {
                    declaration.loaders = release.compatibility.loaders.clone();
                }
            }
        }
    }
    declarations
}

pub(crate) async fn refresh_snapshot(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    refresh_snapshot_with_verification(state, instance_id, cancel, true).await
}

pub(crate) async fn refresh_install_snapshot(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    refresh_snapshot_with_verification(state, instance_id, cancel, false).await
}

async fn refresh_snapshot_with_verification(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    verify_all: bool,
) -> ResourceResult<()> {
    use crate::state::instances::adapters::sqlite::content_rows;
    use crate::state::{
        ContentOwnershipKind, ContentRequirement, ContentSourceKind,
    };
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if verify_all {
        crate::state::instances::commands::sync_content_files_for_operation(
            instance_id,
            state,
        )
        .await?;
    } else {
        crate::state::instances::commands::sync_content_files(
            instance_id,
            state,
        )
        .await?;
    }
    let _writer = state.acquire_install_db_permit().await?;
    let _lock = state.lock_instance_content_exclusive(instance_id).await;
    let scope = crate::state::instances::commands::resolve_content_scope(
        instance_id,
        None,
        state,
    )
    .await?;
    let mut tx = state.pool.begin().await.map_err(plans::db)?;
    let rows: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT file.id, file.relative_path, file.enabled FROM instance_files file WHERE file.instance_id = ? AND file.missing = 0 AND NOT EXISTS (SELECT 1 FROM instance_content_entries entry WHERE entry.content_set_id = ? AND entry.file_id = file.id)"
    ).bind(instance_id).bind(&scope.content_set_id).fetch_all(&mut *tx).await.map_err(plans::db)?;
    let mut changed = false;
    for (id, path, enabled) in rows {
        let Some(kind) = crate::state::instances::adapters::filesystem::project_type_from_relative_path(&path) else { continue };
        if !matches!(
            kind,
            crate::state::ProjectType::Mod
                | crate::state::ProjectType::ResourcePack
                | crate::state::ProjectType::ShaderPack
                | crate::state::ProjectType::DataPack
        ) {
            continue;
        }
        content_rows::upsert_content_entry_from_parts_in_transaction(
            content_rows::UpsertContentEntry {
                instance_id,
                content_set_id: &scope.content_set_id,
                file_id: Some(&id),
                project_type: kind,
                source_kind: ContentSourceKind::Local,
                ownership_kind: ContentOwnershipKind::UserAdded,
                auto_dependency: false,
                server_requirement: ContentRequirement::Unknown,
                client_requirement: ContentRequirement::Unknown,
                enabled: enabled != 0,
            },
            &mut tx,
        )
        .await?;
        changed = true;
    }
    changed |= content_rows::repair_pack_members_in_transaction(
        &scope.content_set_id,
        &mut tx,
    )
    .await?;
    if changed {
        content_rows::bump_content_set_revision_in_transaction(
            &scope.content_set_id,
            &mut tx,
        )
        .await?;
    }
    tx.commit().await.map_err(plans::db)?;
    Ok(())
}

pub(crate) async fn hydrate(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<Vec<InstalledResource>> {
    Ok(hydrate_with_failures(state, instance_id, cancel)
        .await?
        .entries)
}

pub(crate) struct HydratedInstalled {
    pub entries: Vec<InstalledResource>,
    pub failures: Vec<(InstalledEntryId, String)>,
}

pub(crate) async fn hydrate_with_failures(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<HydratedInstalled> {
    refresh_snapshot(state, instance_id, cancel).await?;
    hydrate_selected(state, instance_id, cancel, None).await
}

pub(crate) async fn hydrate_selected(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    selected: Option<&[InstalledEntryId]>,
) -> ResourceResult<HydratedInstalled> {
    let service = state.resources().await?;
    hydrate_selected_with_service(state, instance_id, cancel, selected, service)
        .await
}

pub(crate) async fn hydrate_selected_with_service(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    selected: Option<&[InstalledEntryId]>,
    service: &std::sync::Arc<ResourceService>,
) -> ResourceResult<HydratedInstalled> {
    hydrate_entries_with_service(
        state,
        instance_id,
        cancel,
        selected,
        false,
        service,
    )
    .await
}

pub(crate) async fn hydrate_update_batch_with_service(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    entry_ids: &[InstalledEntryId],
    service: &std::sync::Arc<ResourceService>,
) -> ResourceResult<HydratedInstalled> {
    hydrate_entries_with_service(
        state,
        instance_id,
        cancel,
        Some(entry_ids),
        true,
        service,
    )
    .await
}

async fn hydrate_entries_with_service(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    selected: Option<&[InstalledEntryId]>,
    only_selected: bool,
    service: &std::sync::Arc<ResourceService>,
) -> ResourceResult<HydratedInstalled> {
    let revision = plans::instance_revision(&state.pool, instance_id).await?;
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
    let mut entries = plans::installed(&state.pool, instance_id).await?;
    if only_selected {
        entries.retain(|entry| {
            selected.is_some_and(|ids| ids.contains(&entry.entry_id))
        });
    }
    let mut identities = Vec::new();
    let mut indices = Vec::new();
    let mut failures = Vec::new();
    let mut persisted = Vec::new();
    let mut verified = std::collections::HashMap::new();
    let saved_records: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>("SELECT exact.entry_id, exact.artifact FROM resource_installed_artifacts exact JOIN instance_content_entries entry ON entry.id = exact.entry_id WHERE entry.instance_id = ?")
        .bind(instance_id).fetch_all(&state.pool).await.map_err(plans::db)?.into_iter().collect();
    for (index, entry) in entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| !entry.missing)
    {
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        if (only_selected
            || selected.is_none_or(|ids| !ids.contains(&entry.entry_id)))
            && saved_records
                .get(entry.entry_id.as_str())
                .and_then(|payload| {
                    serde_json::from_str::<ResourceArtifact>(payload).ok()
                })
                .is_some_and(|artifact| {
                    super::valid_artifact(&artifact)
                        && identity::exact_integrity(
                            &entry.integrity,
                            &artifact.integrity,
                        )
                        && entry.artifact.as_ref() == Some(&artifact.reference)
                })
        {
            // The snapshot and exact artifact already agree. Installing an
            // unrelated project must not re-fetch this entry's presentation or
            // probe its counterpart on another source.
            continue;
        }
        let bytes =
            match super::execution::checked_path(&root, &entry.relative_path) {
                Ok(path) => tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled), bytes = tokio::fs::read(path) => bytes }
                    .map(bytes::Bytes::from)
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error.to_string()),
            };
        let bytes = match bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                failures.push((
                    entry.entry_id.clone(),
                    format!("{}: {error}", entry.relative_path),
                ));
                continue;
            }
        };
        let hash_bytes = bytes.clone();
        let hash_cancel = cancel.clone();
        let (mut hashes, sha1, fingerprint) =
            tokio::task::spawn_blocking(move || {
                use sha2::Digest;
                let mut sha512 = sha2::Sha512::new();
                let mut sha256 = sha2::Sha256::new();
                let mut sha1 = sha1_smol::Sha1::new();
                for part in hash_bytes.chunks(256 * 1024) {
                    if hash_cancel.is_cancelled() {
                        return Err(ResourceError::Canceled);
                    }
                    sha512.update(part);
                    sha256.update(part);
                    sha1.update(part);
                }
                let hashes = vec![
                    ContentHash {
                        algorithm: HashAlgorithm::Sha512,
                        value: format!("{:x}", sha512.finalize()),
                    },
                    ContentHash {
                        algorithm: HashAlgorithm::Sha256,
                        value: format!("{:x}", sha256.finalize()),
                    },
                ];
                Ok((
                    hashes,
                    sha1.digest().to_string(),
                    crate::util::fingerprint::compute_fingerprint(&hash_bytes),
                ))
            })
            .await
            .map_err(|error| {
                ResourceError::InvalidRequest(error.to_string())
            })??;
        if entry.integrity.size != Some(bytes.len() as u64)
            || !entry.integrity.hashes.iter().any(|hash| {
                hash.algorithm == HashAlgorithm::Sha1
                    && hash.value.eq_ignore_ascii_case(&sha1)
            })
        {
            failures.push((
                entry.entry_id.clone(),
                format!(
                    "{}: installed files changed; refresh content first",
                    entry.relative_path
                ),
            ));
            continue;
        }
        hashes.push(ContentHash {
            algorithm: HashAlgorithm::Sha1,
            value: sha1,
        });
        let saved = saved_records.get(entry.entry_id.as_str()).cloned();
        if let Some(artifact) = saved
            .and_then(|value| {
                serde_json::from_str::<ResourceArtifact>(&value).ok()
            })
            .filter(|artifact| {
                super::valid_artifact(artifact)
                    && identity::exact_integrity(
                        &ArtifactIntegrity {
                            size: Some(bytes.len() as u64),
                            hashes: hashes.clone(),
                        },
                        &artifact.integrity,
                    )
            })
        {
            verified.insert(index, artifact);
        }
        identities.push(LocalArtifactIdentity {
            integrity: ArtifactIntegrity {
                size: Some(bytes.len() as u64),
                hashes,
            },
            fingerprint: Some(fingerprint),
        });
        indices.push(index);
    }
    let sources: Vec<_> = service
        .sources()
        .into_iter()
        .filter(|source| {
            source.available
                && matches!(source.id.as_str(), "modrinth" | "curseforge")
        })
        .map(|source| source.id)
        .collect();
    let recognized = service
        .identify_many(&sources, &identities, CachePolicy::UseFresh, cancel)
        .await;
    let mut projects: std::collections::HashSet<_> = recognized
        .iter()
        .flat_map(|recognition| {
            recognition
                .matches
                .iter()
                .map(|artifact| artifact.reference.release.project.clone())
        })
        .collect();
    projects.extend(
        verified
            .values()
            .map(|artifact| artifact.reference.release.project.clone()),
    );
    for source in &sources {
        let queries = projects
            .iter()
            .filter(|project| &project.source == source)
            .map(|project| super::providers::ProviderQuery::Project {
                id: project.project_id.clone(),
                slug: None,
            })
            .collect();
        service
            .query_many(source, queries, CachePolicy::UseFresh, cancel)
            .await;
    }
    for ((index, local), recognition) in
        indices.into_iter().zip(identities).zip(recognized)
    {
        let entry = &mut entries[index];
        let refs = crate::state::instances::adapters::sqlite::content_rows::get_content_provider_refs(entry.entry_id.as_str(), &state.pool).await?;
        let origin = crate::state::instances::adapters::sqlite::content_rows::get_content_origin_provider(entry.entry_id.as_str(), &state.pool).await?;
        let mut matches = recognition.matches;
        if let Some(artifact) = verified.remove(&index)
            && !matches
                .iter()
                .any(|other| other.reference == artifact.reference)
        {
            matches.push(artifact);
        }
        // Exact legacy release references can corroborate files absent from a
        // hash index. They must agree with current local bytes.
        for reference in &refs {
            if let Some(release_id) = reference.database_release_id() {
                let release_ref = SourceReleaseRef {
                    project: SourceProjectRef {
                        source: SourceId::new(reference.provider().as_str())?,
                        project_id: SourceProjectId::new(
                            reference.database_project_id(),
                        )?,
                    },
                    release_id: SourceReleaseId::new(release_id)?,
                };
                if matches
                    .iter()
                    .any(|artifact| artifact.reference.release == release_ref)
                {
                    continue;
                }
                if let Ok(release) =
                    service.release(&release_ref, None, cancel).await
                {
                    for artifact in
                        release.artifacts.into_iter().filter(|artifact| {
                            artifact.role == ArtifactRole::Primary
                                && identity::exact_integrity(
                                    &local.integrity,
                                    &artifact.integrity,
                                )
                        })
                    {
                        if !matches
                            .iter()
                            .any(|other| other.reference == artifact.reference)
                        {
                            matches.push(artifact);
                        }
                    }
                }
            }
        }
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        let selected = entry
            .artifact
            .as_ref()
            .and_then(|reference| {
                matches
                    .iter()
                    .find(|artifact| &artifact.reference == reference)
            })
            .or_else(|| {
                let origin_matches: Vec<_> = matches
                    .iter()
                    .filter(|artifact| {
                        origin.is_some_and(|origin| {
                            artifact.reference.release.project.source.as_str()
                                == origin.as_str()
                        })
                    })
                    .collect();
                if origin_matches.len() == 1 {
                    Some(origin_matches[0])
                } else if origin.is_none() && !recognition.ambiguous {
                    matches.first()
                } else {
                    None
                }
            })
            .cloned();
        if selected.is_none() {
            entry.artifact = None;
            entry.known_artifacts.clear();
            for (source, error) in recognition.failures {
                failures.push((
                    entry.entry_id.clone(),
                    format!(
                        "{} ({}): {error}",
                        entry.relative_path,
                        source.as_str()
                    ),
                ));
            }
            continue;
        }
        let mut selected = selected.unwrap();
        if !selected
            .integrity
            .hashes
            .iter()
            .any(|hash| hash.algorithm == HashAlgorithm::Sha1)
        {
            selected.integrity.hashes.extend(
                local
                    .integrity
                    .hashes
                    .iter()
                    .filter(|hash| hash.algorithm == HashAlgorithm::Sha1)
                    .cloned(),
            );
        }
        let metadata = match service
            .project_document(&selected.reference.release.project, None, cancel)
            .await
        {
            Ok(document) => document.metadata,
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => {
                failures.push((
                    entry.entry_id.clone(),
                    format!("{}: {error}", entry.relative_path),
                ));
                entry.artifact = None;
                continue;
            }
        };
        // Hash matches below can confirm counterparts without a separate
        // project-history probe for every file in the instance.
        if !metadata.types.contains(&entry.resource_type) {
            entry.artifact = None;
            continue;
        }
        if let Some(store) = service.store() {
            let id = store
                .ensure_project(&metadata.project, &metadata.types)
                .await?;
            entry.resource_id = Some(id);
            for matched in matches.iter().filter(|_| !recognition.ambiguous) {
                if matched.reference.release.project != metadata.project
                    && store.lookup(&matched.reference.release.project).await?
                        != entry.resource_id
                {
                    let other = match service
                        .project_document(
                            &matched.reference.release.project,
                            None,
                            cancel,
                        )
                        .await
                    {
                        Ok(document) => document.metadata,
                        Err(ResourceError::Canceled) => {
                            return Err(ResourceError::Canceled);
                        }
                        Err(_) => continue,
                    };
                    store
                        .confirm_projects(&metadata, &other, &selected, matched)
                        .await?;
                }
            }
            entry.resource_id = store.lookup(&metadata.project).await?;
        }
        entry.artifact = Some(selected.reference.clone());
        entry.known_artifacts = matches
            .iter()
            .filter(|artifact| {
                !recognition.ambiguous
                    || artifact.reference == selected.reference
            })
            .map(|artifact| artifact.reference.clone())
            .collect();
        entry.integrity = local.integrity;
        entry.original_file_name = selected.file_name.clone();
        persisted.push((entry.entry_id.clone(), selected, entry.scope.clone()));
    }
    for (id, _) in &failures {
        if let Some(entry) =
            entries.iter_mut().find(|entry| &entry.entry_id == id)
        {
            entry.artifact = None;
            entry.known_artifacts.clear();
        }
    }
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    let _writer = state.acquire_install_db_permit().await?;
    let _lock = state.lock_instance_content_exclusive(instance_id).await;
    if plans::instance_revision(&state.pool, instance_id).await? != revision {
        return Err(ResourceError::InvalidRequest(
            "instance changed while verifying installed resources".into(),
        ));
    }
    let mut tx = state.pool.begin().await.map_err(plans::db)?;
    for (id, artifact, scope) in persisted {
        let previous: Option<String> = sqlx::query_scalar("SELECT artifact FROM resource_installed_artifacts WHERE entry_id = ?").bind(id.as_str()).fetch_optional(&mut *tx).await.map_err(plans::db)?;
        let declared_compatibility = previous
            .and_then(|payload| {
                serde_json::from_str::<InstalledArtifactRecord>(&payload).ok()
            })
            .filter(|previous| {
                previous.artifact.reference == artifact.reference
                    && identity::exact_integrity(
                        &previous.artifact.integrity,
                        &artifact.integrity,
                    )
            })
            .and_then(|previous| previous.declared_compatibility);
        let record = InstalledArtifactRecord {
            artifact,
            declared_compatibility,
        };
        sqlx::query("INSERT INTO resource_installed_artifacts(entry_id, artifact, scope) VALUES (?, ?, ?) ON CONFLICT(entry_id) DO UPDATE SET artifact = excluded.artifact, scope = excluded.scope").bind(id.as_str()).bind(serde_json::to_string(&record)?).bind(serde_json::to_string(&scope)?).execute(&mut *tx).await.map_err(plans::db)?;
    }
    tx.commit().await.map_err(plans::db)?;
    Ok(HydratedInstalled { entries, failures })
}

pub async fn list(
    instance_id: String,
    cancel: &CancellationToken,
) -> ResourceResult<Vec<InstalledResource>> {
    let state = crate::State::get().await?;
    hydrate(&state, &instance_id, cancel).await
}

pub async fn prepare_entries(
    instance_id: String,
    intent: ResourceOperationIntent,
    entry_ids: Vec<InstalledEntryId>,
    release: Option<SourceReleaseRef>,
    install_dependencies: bool,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceOperationPlan> {
    prepare_entries_with_progress(
        instance_id,
        intent,
        entry_ids,
        release,
        install_dependencies,
        cancel,
        None,
        true,
    )
    .await
}

pub async fn prepare_entries_with_progress(
    instance_id: String,
    intent: ResourceOperationIntent,
    entry_ids: Vec<InstalledEntryId>,
    release: Option<SourceReleaseRef>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    progress: plans::OperationProgress<'_>,
    resolve_dependencies: bool,
) -> ResourceResult<ResourceOperationPlan> {
    if intent == ResourceOperationIntent::Install || entry_ids.is_empty() {
        return Err(ResourceError::InvalidRequest(
            "select installed entries to change".into(),
        ));
    }
    let state = crate::State::get().await?;
    refresh_snapshot(&state, &instance_id, cancel).await?;
    let hydrated =
        hydrate_selected(&state, &instance_id, cancel, Some(&entry_ids))
            .await?;
    if let Some((_, error)) = hydrated
        .failures
        .iter()
        .find(|(id, _)| entry_ids.contains(id))
    {
        return Err(ResourceError::InvalidRequest(error.clone()));
    }
    let entries = hydrated.entries;
    let mut selections = Vec::new();
    for id in entry_ids {
        let entry = entries
            .iter()
            .find(|entry| entry.entry_id == id)
            .ok_or_else(|| {
                ResourceError::InvalidRequest(
                    "installed entry is missing".into(),
                )
            })?;
        let artifact = entry.artifact.as_ref().ok_or_else(|| {
            ResourceError::InvalidRequest(format!(
                "cannot verify the current artifact: {}",
                entry.relative_path
            ))
        })?;
        selections.push(ResourceSelection {
            id: SelectionId::new(id.as_str())?,
            resource: entry
                .resource_id
                .as_ref()
                .map(|id| ResourceRef::Resource { id: id.clone() })
                .unwrap_or_else(|| ResourceRef::Source {
                    project: artifact.release.project.clone(),
                }),
            release: release
                .as_ref()
                .map(|reference| ReleaseSelection::Release {
                    reference: reference.clone(),
                })
                .unwrap_or(ReleaseSelection::Automatic),
            resource_type: entry.resource_type,
            scope: entry.scope.clone(),
            entry_id: Some(id),
        });
    }
    plans::prepare_with_installed(
        instance_id,
        intent,
        selections,
        install_dependencies,
        cancel,
        Some(&entries),
        progress,
        resolve_dependencies,
    )
    .await
}
