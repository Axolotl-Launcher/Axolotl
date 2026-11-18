use super::{identity::LocalArtifactIdentity, *};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::resource_entry_graphs::ResourceEntryGraphs;
use crate::state::store::resource_installed_artifacts::{
    ResourceInstalledArtifactRecord, ResourceInstalledArtifacts,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstalledArtifactRecord {
    #[serde(flatten)]
    pub artifact: ResourceArtifact,
    #[serde(default)]
    pub declared_compatibility: Option<ReleaseCompatibility>,
}

pub(super) fn legacy_modrinth_project(project: &SourceProjectRef) -> bool {
    project.source == SourceId::modrinth()
        && !providers::stable_modrinth_id(project.project_id.as_str())
}

pub(super) enum LegacyModrinthIdentity {
    Recorded {
        project: String,
        release: Option<String>,
    },
    Ambiguous,
}

pub(super) fn legacy_modrinth_artifact(
    service: &ResourceService,
    artifact: &SourceArtifactRef,
) -> bool {
    let release = &artifact.release;
    release.project.source == SourceId::modrinth()
        && service.rejects_modrinth_identity(
            release.project.project_id.as_str(),
            Some(release.release_id.as_str()),
        )
}

pub(super) fn legacy_modrinth_identity(
    service: &ResourceService,
    existing: Option<&SourceArtifactRef>,
    refs: &[crate::state::ContentProviderRef],
    origin: Option<&SourceId>,
) -> Option<LegacyModrinthIdentity> {
    if let Some(existing) = existing {
        return legacy_modrinth_artifact(service, existing).then(|| {
            LegacyModrinthIdentity::Recorded {
                project: existing
                    .release
                    .project
                    .project_id
                    .as_str()
                    .to_owned(),
                release: Some(existing.release.release_id.as_str().to_owned()),
            }
        });
    }
    let refs = refs.iter().filter(|reference| {
        origin.is_none_or(|source| {
            source.as_str() == reference.provider().as_str()
        })
    });
    let (project, release) = refs.clone().find_map(|reference| {
        if reference.provider() != crate::state::ContentProvider::Modrinth {
            return None;
        }
        let project = reference.database_project_id();
        let release = reference.database_release_id();
        service
            .rejects_modrinth_identity(&project, release.as_deref())
            .then_some((project, release))
    })?;
    Some(if refs.count() == 1 {
        LegacyModrinthIdentity::Recorded { project, release }
    } else {
        LegacyModrinthIdentity::Ambiguous
    })
}

impl LegacyModrinthIdentity {
    pub(super) async fn verify(
        &self,
        service: &std::sync::Arc<ResourceService>,
        local: &LocalArtifactIdentity,
        policy: CachePolicy,
        cancel: &CancellationToken,
    ) -> ResourceResult<ResourceArtifact> {
        let unverifiable = || {
            ResourceError::InvalidResponse(
                "cannot verify the recorded Modrinth identity against the installed file".into(),
            )
        };
        let Self::Recorded { project, release } = self else {
            return Err(unverifiable());
        };
        let source = SourceId::modrinth();
        let release_id = release
            .as_deref()
            .filter(|id| providers::stable_modrinth_id(id));
        let project = if providers::stable_modrinth_id(project) {
            providers::source_project(source.clone(), project.as_str())?
        } else {
            if release_id.is_none()
                || project.is_empty()
                || !project.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
                })
            {
                return Err(unverifiable());
            }
            let document = service
                .locate_with_policy(&source, project.clone(), cancel, policy)
                .await?;
            if document.metadata.project.source != source
                || !document
                    .metadata
                    .slug
                    .as_ref()
                    .is_some_and(|slug| slug.eq_ignore_ascii_case(project))
                || legacy_modrinth_project(&document.metadata.project)
            {
                return Err(unverifiable());
            }
            document.metadata.project
        };
        let artifacts = if let Some(release_id) = release_id {
            let reference =
                providers::release_ref(project.clone(), release_id)?;
            service
                .release_with_policy(&reference, None, cancel, policy)
                .await?
                .artifacts
        } else {
            let mut recognized = service
                .identify_many(
                    &[source],
                    std::slice::from_ref(local),
                    policy,
                    cancel,
                )
                .await;
            let recognition = recognized.pop().ok_or_else(unverifiable)?;
            if let Some((_, error)) = recognition.failures.into_iter().next() {
                return Err(error);
            }
            recognition.matches
        };
        let mut exact = artifacts.into_iter().filter(|artifact| {
            artifact.role == ArtifactRole::Primary
                && artifact.reference.release.project == project
                && providers::stable_modrinth_id(
                    artifact.reference.release.release_id.as_str(),
                )
                && super::valid_artifact(artifact)
                && identity::exact_integrity(
                    &local.integrity,
                    &artifact.integrity,
                )
        });
        let artifact = exact.next().ok_or_else(unverifiable)?;
        if exact.any(|other| other.reference != artifact.reference) {
            return Err(unverifiable());
        }
        Ok(artifact)
    }
}

/// The artifact payload stored for an entry, as written, so a caller can
/// compare integrity before replacing it.
pub(crate) async fn installed_artifact_payload(
    entry_id: &str,
    conn: &mut sqlx::SqliteConnection,
) -> crate::Result<Option<String>> {
    let key = entry_id.to_owned();
    Ok(ResourceInstalledArtifacts::read(conn, &key)
        .await?
        .map(|record| record.artifact))
}

/// Records the artifact an entry installs, replacing what it held before.
pub(crate) async fn set_installed_artifact<T>(
    entry_id: &str,
    payload: &T,
    scope: &InstallScope,
    conn: &mut sqlx::SqliteConnection,
) -> crate::Result<()>
where
    T: serde::Serialize + ?Sized,
{
    let key = entry_id.to_owned();
    let previous = ResourceInstalledArtifacts::read(conn, &key).await?;
    let mut artifact = serde_json::to_value(payload)?;
    let mut scope_value = serde_json::to_value(scope)?;
    if let Some(previous) = previous {
        let old: InstalledArtifactRecord =
            crate::state::store::compatibility::decode_payload(
                &previous.artifact,
            )?;
        let current: InstalledArtifactRecord =
            serde_json::from_value(artifact.clone())?;
        if old.artifact.reference == current.artifact.reference
            && identity::exact_integrity(
                &old.artifact.integrity,
                &current.artifact.integrity,
            )
        {
            let mut raw: serde_json::Value =
                serde_json::from_str(&previous.artifact)?;
            patch_known_fields(
                &mut raw,
                &serde_json::to_value(&old)?,
                &artifact,
            );
            artifact = raw;
            let old_scope: InstallScope =
                crate::state::store::compatibility::decode_payload(
                    &previous.scope,
                )?;
            let mut raw_scope: serde_json::Value =
                serde_json::from_str(&previous.scope)?;
            patch_known_fields(
                &mut raw_scope,
                &serde_json::to_value(old_scope)?,
                &scope_value,
            );
            scope_value = raw_scope;
        }
    }
    let record = ResourceInstalledArtifactRecord {
        artifact: serde_json::to_string(&artifact)?,
        scope: serde_json::to_string(&scope_value)?,
    };

    let key = entry_id.to_owned();
    ResourceInstalledArtifacts::write(conn, &key, Some(&record)).await
}

/// Leave raw values untouched when their known interpretation did not change.
/// This also retains unknown fields inside arrays and nested records.
fn patch_known_fields(
    raw: &mut serde_json::Value,
    before: &serde_json::Value,
    after: &serde_json::Value,
) {
    if before == after {
        return;
    }
    if let (Some(raw), Some(before), Some(after)) =
        (raw.as_object_mut(), before.as_object(), after.as_object())
    {
        for (key, value) in after {
            match (raw.get_mut(key), before.get(key)) {
                (Some(raw), Some(before)) => {
                    patch_known_fields(raw, before, value)
                }
                _ => {
                    raw.insert(key.clone(), value.clone());
                }
            }
        }
    } else {
        *raw = after.clone();
    }
}

/// Drops the artifact an entry no longer installs.
pub(crate) async fn delete_installed_artifact(
    entry_id: &str,
    conn: &mut sqlx::SqliteConnection,
) -> crate::Result<()> {
    let key = entry_id.to_owned();
    ResourceInstalledArtifacts::write(conn, &key, None).await
}

/// Drops every resource row an entry leaves behind when it goes away.
pub(crate) async fn delete_entry_resource_traces(
    entry_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<()> {
    delete_installed_artifact(entry_id, tx).await?;
    let key = entry_id.to_owned();
    ResourceEntryGraphs::delete_in(tx, &key).await
}

pub(crate) async fn attach_compatibility(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
    game_version: &str,
    loader: &str,
    items: &mut [crate::state::instances::InstanceContentSnapshotItem],
    service: Option<&std::sync::Arc<ResourceService>>,
) -> crate::Result<()> {
    let rows = ResourceInstalledArtifacts::with_file_state(
        &Handle::new(pool.clone()),
        instance_id,
    )
    .await?;
    let mut parsed = crate::state::store::content_evidence::parsed_for_entries(
        pool,
        instance_id,
    )
    .await?
    .into_iter()
    .collect::<std::collections::HashMap<_, _>>();
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
        .map(|(id, sha1, size, _legacy_local, artifact)| {
            let record = artifact
                .and_then(|payload| {
                    serde_json::from_str::<InstalledArtifactRecord>(&payload)
                        .ok()
                })
                .filter(|record| {
                    !legacy_modrinth_project(
                        &record.artifact.reference.release.project,
                    )
                });
            let local = parsed.remove(&id);
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
        .enrich_many(&projects)
        .await;
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
                    item.capabilities.can_change_version = item.project_type
                        != crate::state::ProjectType::Schematic;
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
                game_versions: metadata
                    .for_loader(loader)
                    .minecraft_version
                    .clone()
                    .into_iter()
                    .collect(),
                loaders: metadata.supported_loaders(),
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
) -> ResourceResult<bool> {
    refresh_snapshot_with_consistency(state, instance_id, cancel, true).await
}

pub(crate) async fn refresh_operation_snapshot(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    refresh_snapshot_with_consistency(state, instance_id, cancel, false)
        .await
        .map(|_| ())
}

pub(crate) async fn refresh_install_snapshot(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
) -> ResourceResult<()> {
    refresh_snapshot_with_consistency(state, instance_id, cancel, false)
        .await
        .map(|_| ())
}

async fn refresh_snapshot_with_consistency(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    allow_committed_snapshot: bool,
) -> ResourceResult<bool> {
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if allow_committed_snapshot {
        let outcome =
            crate::state::instances::commands::sync_content_files_for_display(
                instance_id,
                state,
            )
            .await?;
        if !outcome.stable {
            return Ok(false);
        }
    } else {
        crate::state::instances::commands::sync_content_files(
            instance_id,
            state,
        )
        .await?;
    }
    let _writer = state.acquire_install_db_permit().await?;
    let _lock = state.lock_instance_content_exclusive(instance_id).await;
    crate::state::store::instance_content_files::set_discovered_entries(
        &state.data,
        instance_id,
    )
    .await?;
    Ok(true)
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
    refresh_operation_snapshot(state, instance_id, cancel).await?;
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

pub(super) async fn installed_file_identity(
    state: &crate::State,
    instance_id: &str,
    root: &std::path::Path,
    entry: &InstalledResource,
    cache: &super::cache::ResourceCache,
    reuse: bool,
    cancel: &CancellationToken,
) -> ResourceResult<LocalArtifactIdentity> {
    use crate::state::store::content_file_fingerprints::ContentFileFingerprints;

    let generation = cache.generation().await;
    let content_generation =
        if std::ptr::eq(cache, state.resource_cache().as_ref()) {
            generation
        } else {
            state.resource_cache().generation().await
        };
    let previous = ContentFileFingerprints::get_current(
        &state.data,
        instance_id,
        &entry.relative_path,
    )
    .await?;
    let result = state
        .file_fingerprints
        .resolve(
            root,
            &entry.relative_path,
            previous.as_ref(),
            !reuse,
            cancel,
        )
        .await?;
    if !identity::exact_integrity(&entry.integrity, &result.identity.integrity)
    {
        return Err(ResourceError::InvalidRequest(
            "installed files changed; refresh content first".into(),
        ));
    }
    if let Some(record) = &result.record {
        ContentFileFingerprints::set_current_at(
            &state.data,
            instance_id,
            record,
            content_generation,
        )
        .await?;
    }
    cache
        .store_local_identity(&result.identity, generation)
        .await;
    Ok(result.identity)
}

async fn hydrate_entries_with_service(
    state: &crate::State,
    instance_id: &str,
    cancel: &CancellationToken,
    selected: Option<&[InstalledEntryId]>,
    only_selected: bool,
    service: &std::sync::Arc<ResourceService>,
) -> ResourceResult<HydratedInstalled> {
    use crate::state::instances::adapters::sqlite::content_entries;

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
    let saved_records: std::collections::HashMap<String, String> =
        ResourceInstalledArtifacts::payloads_for_instance(
            &state.data,
            instance_id,
        )
        .await?
        .into_iter()
        .collect();
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
                        && !legacy_modrinth_artifact(
                            service,
                            &artifact.reference,
                        )
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
        let local = match installed_file_identity(
            state,
            instance_id,
            &root,
            entry,
            service.cache(),
            true,
            cancel,
        )
        .await
        {
            Ok(identity) => identity,
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled);
            }
            Err(error) => {
                failures.push((
                    entry.entry_id.clone(),
                    format!("{}: {error}", entry.relative_path),
                ));
                continue;
            }
        };
        let saved = saved_records.get(entry.entry_id.as_str()).cloned();
        if let Some(artifact) = saved
            .and_then(|value| {
                serde_json::from_str::<ResourceArtifact>(&value).ok()
            })
            .filter(|artifact| {
                super::valid_artifact(artifact)
                    && !legacy_modrinth_artifact(service, &artifact.reference)
                    && identity::exact_integrity(
                        &local.integrity,
                        &artifact.integrity,
                    )
            })
        {
            verified.insert(index, artifact);
        }
        identities.push(local);
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
    let preferred = service.preferred_source().await?;
    for ((index, local), recognition) in
        indices.into_iter().zip(identities).zip(recognized)
    {
        let entry = &mut entries[index];
        let refs = match content_entries::get_content_provider_refs(
            entry.entry_id.as_str(),
            &state.pool,
        )
        .await
        .map_err(ResourceError::from)
        {
            Ok(refs) => refs,
            Err(error @ ResourceError::InvalidRequest(_)) => {
                failures.push((
                    entry.entry_id.clone(),
                    format!("{}: {error}", entry.relative_path),
                ));
                continue;
            }
            Err(error) => return Err(error),
        };
        let origin = content_entries::get_content_origin_provider(
            entry.entry_id.as_str(),
            &state.pool,
        )
        .await?;
        let origin = origin
            .as_ref()
            .map(|origin| SourceId::new(origin.as_str()))
            .transpose()?;
        let legacy = legacy_modrinth_identity(
            service,
            entry.artifact.as_ref(),
            &refs,
            origin.as_ref(),
        );
        let mut pinned = entry.artifact.clone();
        let mut ambiguous = recognition.ambiguous;
        let mut matches = recognition.matches;
        if let Some(legacy) = &legacy {
            match legacy
                .verify(service, &local, CachePolicy::UseFresh, cancel)
                .await
            {
                Ok(artifact) => {
                    pinned = Some(artifact.reference.clone());
                    matches = vec![artifact];
                    ambiguous = false;
                }
                Err(
                    error @ (ResourceError::Canceled
                    | ResourceError::Database(_)),
                ) => return Err(error),
                Err(error) => {
                    failures.push((
                        entry.entry_id.clone(),
                        format!("{}: {error}", entry.relative_path),
                    ));
                    continue;
                }
            }
        }
        if legacy.is_none()
            && let Some(artifact) = verified.remove(&index)
            && !matches
                .iter()
                .any(|other| other.reference == artifact.reference)
        {
            matches.push(artifact);
        }
        let failure_count = failures.len();
        // Exact legacy release references can corroborate files absent from a
        // hash index. They must agree with current local bytes.
        for reference in refs.iter().filter(|_| legacy.is_none()) {
            if let Some(release_id) = reference.database_release_id() {
                let Ok(release_ref) = providers::source_project(
                    SourceId::new(reference.provider().as_str())?,
                    reference.database_project_id(),
                )
                .and_then(|project| {
                    providers::release_ref(project, release_id)
                }) else {
                    continue;
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
        let selected = match service
            .select_recognized_artifact(
                &matches,
                pinned.as_ref(),
                origin.as_ref(),
                &preferred,
                entry.resource_type,
                ambiguous,
                CachePolicy::UseFresh,
                cancel,
            )
            .await
        {
            Ok(selected) => selected,
            Err(error) if identity::recognition_source_failure(&error) => {
                failures.push((
                    entry.entry_id.clone(),
                    format!("{}: {error}", entry.relative_path),
                ));
                None
            }
            Err(error) => return Err(error),
        };
        if selected.is_none() {
            entry.artifact = None;
            entry.known_artifacts.clear();
            if legacy.is_some() {
                if failures.len() == failure_count {
                    failures.push((
                        entry.entry_id.clone(),
                        format!("{}: cannot verify the recorded Modrinth project for this content type", entry.relative_path),
                    ));
                }
                continue;
            }
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
        let (mut selected, metadata) = selected.unwrap();
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
        if let Some(store) = service.store() {
            let id = store
                .ensure_project(&metadata.project, &metadata.types)
                .await?;
            entry.resource_id = Some(id);
            for matched in matches.iter().filter(|_| !ambiguous) {
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
                        Err(
                            error @ (ResourceError::Canceled
                            | ResourceError::Database(_)),
                        ) => return Err(error),
                        Err(_) => continue,
                    };
                    identity::try_confirm_projects(
                        store, &metadata, &other, &selected, matched, cancel,
                    )
                    .await?;
                }
            }
            entry.resource_id = store.lookup(&metadata.project).await?;
        }
        entry.artifact = Some(selected.reference.clone());
        // Planning uses known artifacts as project identity, not just byte
        // evidence. An unconfirmed counterpart must not satisfy dependencies.
        entry.known_artifacts = vec![selected.reference.clone()];
        for artifact in &matches {
            if ambiguous || artifact.reference == selected.reference {
                continue;
            }
            let same_project =
                artifact.reference.release.project == metadata.project;
            let confirmed = if let Some(store) = service.store() {
                entry.resource_id.is_some()
                    && store.lookup(&artifact.reference.release.project).await?
                        == entry.resource_id
            } else {
                false
            };
            if same_project || confirmed {
                entry.known_artifacts.push(artifact.reference.clone());
            }
        }
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
    let (mut tx, tx_diagnostics) =
        crate::state::store::lock_diagnostics::begin(
            &state.pool,
            "BEGIN IMMEDIATE",
        )
        .await
        .map_err(plans::db)?;
    for (id, artifact, scope) in persisted {
        let previous = installed_artifact_payload(id.as_str(), &mut tx).await?;
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
        set_installed_artifact(id.as_str(), &record, &scope, &mut tx).await?;
    }
    tx_diagnostics
        .observe("commit_wait", tx.commit())
        .await
        .map_err(plans::db)?;
    tx_diagnostics.complete("committed");
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
    plans::bounded_preparation(
        cancel,
        std::time::Duration::from_secs(15 * 60),
        |cancel| async move {
            prepare_entries_inner(
                instance_id,
                intent,
                entry_ids,
                release,
                install_dependencies,
                &cancel,
                progress,
                resolve_dependencies,
            )
            .await
        },
    )
    .await
}

async fn prepare_entries_inner(
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
    plans::report_phase(
        progress,
        plans::ResourceOperationPhase::RefreshingContent,
    );
    refresh_operation_snapshot(&state, &instance_id, cancel).await?;
    plans::report_phase(
        progress,
        plans::ResourceOperationPhase::IdentifyingInstalled,
    );
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
