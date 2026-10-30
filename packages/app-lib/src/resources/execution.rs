use super::*;
use crate::state::instances::commands::{
    instance_content_root, materialize_verified_project_download_copy,
    resolve_content_scope, restore_project_materialization,
};
use crate::state::store::Collection as _;
use crate::state::store::Handle;
use crate::state::store::resource_entry_graphs::{
    EntryGraphRecord, ResourceEntryGraphs,
};
use crate::state::store::resource_execution_journals::ResourceExecutionJournals;
use crate::state::store::resource_history::{
    ResourceHistory, ResourceHistoryRecord,
};
use crate::state::store::resource_operation_plans::ResourceOperationPlans;
use crate::state::store::resource_pack_unlinks;
use crate::state::store::store_retained_refs as retained_refs;
use crate::state::{
    ContentOwnershipKind, ContentProviderRef, ContentRequirement,
    ContentSourceKind, ProjectType,
};
use crate::util::fetch::{
    self, ContentValidation, DownloadRequest, Integrity, ResourceClass,
};
use futures::{FutureExt, TryStreamExt, stream::FuturesUnordered};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

use crate::state::instances::adapters::sqlite::{
    content_entries, content_sets, content_store_files, files, pack_members,
};

fn input(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}
fn canceled(cancel: &CancellationToken) -> crate::Result<()> {
    if cancel.is_cancelled() {
        Err(input("resource operation canceled"))
    } else {
        Ok(())
    }
}

pub(crate) fn entry_graph(
    plan: &ResourceOperationPlan,
    root: &SelectionId,
) -> (Vec<ResourceDependencyNode>, Vec<ResourceDependencyEdge>) {
    let mut ids = std::collections::HashSet::from([root.clone()]);
    let mut pending = vec![root.clone()];
    while let Some(id) = pending.pop() {
        for edge in plan.edges.iter().filter(|edge| edge.parent == id) {
            if ids.insert(edge.child.clone()) {
                pending.push(edge.child.clone());
            }
        }
    }
    (
        plan.nodes
            .iter()
            .filter(|node| ids.contains(&node.id))
            .cloned()
            .collect(),
        plan.edges
            .iter()
            .filter(|edge| {
                ids.contains(&edge.parent) && ids.contains(&edge.child)
            })
            .cloned()
            .collect(),
    )
}

fn staging_prefix(plan: &ResourceOperationPlan) -> crate::Result<String> {
    let id = uuid::Uuid::parse_str(plan.id.as_str())?;
    Ok(format!("resource-{id}-"))
}

async fn clear_staging(
    plan: &ResourceOperationPlan,
    state: &crate::State,
) -> crate::Result<()> {
    let base = state.directories.caches_dir().join("resource-staging");
    if !base.exists() {
        return Ok(());
    }
    let canonical = std::fs::canonicalize(&base)?;
    let prefix = staging_prefix(plan)?;
    let mut directories = tokio::fs::read_dir(&base).await?;
    while let Some(entry) = directories.next_entry().await? {
        if !entry.file_name().to_string_lossy().starts_with(&prefix)
            || !entry.file_type().await?.is_dir()
        {
            continue;
        }
        let path = std::fs::canonicalize(entry.path())?;
        if path.parent() != Some(canonical.as_path()) {
            return Err(input(
                "resource staging path escaped its cache directory",
            ));
        }
        tokio::fs::remove_dir_all(path).await?;
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JournalFile {
    path: String,
    #[serde(default)]
    previous_path: Option<String>,
    backup: Option<String>,
    old: Option<ArtifactIntegrity>,
    target: ArtifactIntegrity,
}

pub(crate) fn integrity(
    artifact: &ResourceArtifact,
    kind: ResourceType,
) -> Integrity {
    let mut result = Integrity {
        size: artifact.integrity.size,
        content: if matches!(kind, ResourceType::Mod | ResourceType::Plugin) {
            ContentValidation::Jar
        } else {
            ContentValidation::None
        },
        ..Default::default()
    };
    for hash in &artifact.integrity.hashes {
        match hash.algorithm {
            HashAlgorithm::Sha1 => result.sha1 = Some(hash.value.clone()),
            HashAlgorithm::Sha256 => result.sha256 = Some(hash.value.clone()),
            HashAlgorithm::Sha512 => result.sha512 = Some(hash.value.clone()),
            HashAlgorithm::Md5 => result.md5 = Some(hash.value.clone()),
        }
    }
    result
}

pub(crate) fn provider_ref(
    artifact: &ResourceArtifact,
) -> crate::Result<ContentProviderRef> {
    let reference = &artifact.reference;
    ContentProviderRef::from_database(
        reference.release.project.source.as_str(),
        reference.release.project.project_id.as_str(),
        Some(reference.release.release_id.as_str()),
        Some(reference.artifact_id.as_str()),
    )
}

pub(crate) fn project_type(kind: ResourceType) -> crate::Result<ProjectType> {
    match kind {
        ResourceType::Mod | ResourceType::Plugin => Ok(ProjectType::Mod),
        ResourceType::DataPack => Ok(ProjectType::DataPack),
        ResourceType::ResourcePack => Ok(ProjectType::ResourcePack),
        ResourceType::Shader => Ok(ProjectType::ShaderPack),
        _ => Err(input("unsupported resource content type")),
    }
}

pub(crate) async fn matches_file(
    path: &Path,
    expected: &ArtifactIntegrity,
) -> crate::Result<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    if !expected
        .hashes
        .iter()
        .any(|hash| hash.algorithm != HashAlgorithm::Md5)
    {
        return Ok(false);
    }
    let mut check = Integrity {
        size: expected.size,
        ..Default::default()
    };
    for hash in &expected.hashes {
        match hash.algorithm {
            HashAlgorithm::Sha1 => check.sha1 = Some(hash.value.clone()),
            HashAlgorithm::Sha256 => check.sha256 = Some(hash.value.clone()),
            HashAlgorithm::Sha512 => check.sha512 = Some(hash.value.clone()),
            HashAlgorithm::Md5 => check.md5 = Some(hash.value.clone()),
        }
    }
    let actual = fetch::compute_file_integrity(path, &check).await?;
    Ok(expected.size.is_none_or(|size| size == actual.size)
        && fetch::verify_computed_integrity(&check, &actual).is_ok())
}

pub(crate) fn checked_path(
    root: &Path,
    relative: &str,
) -> crate::Result<PathBuf> {
    if !plans::safe_relative_path(relative) {
        return Err(input("invalid resource relative path"));
    }
    let canonical_root = std::fs::canonicalize(root)?;
    let path = relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part));
    let mut probe = path.clone();
    while probe != root {
        match std::fs::symlink_metadata(&probe) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(input("resource paths cannot traverse links"));
            }
            Ok(metadata) => {
                if metadata.is_dir()
                    && !std::fs::canonicalize(&probe)?
                        .starts_with(&canonical_root)
                {
                    return Err(input(
                        "resource directory escaped instance root",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if !probe.pop() {
            return Err(input("resource path escaped instance root"));
        }
    }
    Ok(path)
}

async fn recover_files(
    root: &Path,
    files: &[JournalFile],
) -> crate::Result<()> {
    for file in files.iter().rev() {
        let path = checked_path(root, &file.path)?;
        if let Some(backup) = &file.backup {
            let backup = checked_path(root, backup)?;
            if backup.exists() {
                let old = file.old.as_ref().ok_or_else(|| {
                    input("journal backup has no expected identity")
                })?;
                if !matches_file(&backup, old).await? {
                    return Err(input("resource recovery backup changed"));
                }
                if path.exists()
                    && !matches_file(&path, &file.target).await?
                    && !matches_file(&path, old).await?
                {
                    return Err(input("resource file changed during recovery"));
                }
                let previous = checked_path(
                    root,
                    file.previous_path.as_deref().unwrap_or(&file.path),
                )?;
                if previous != path {
                    if previous.exists()
                        && !matches_file(&previous, old).await?
                    {
                        return Err(input(
                            "original resource path changed during recovery",
                        ));
                    }
                    if path.exists() {
                        tokio::fs::remove_file(&path).await?;
                    }
                }
                restore_project_materialization(&previous, Some(&backup))
                    .await?;
            } else {
                let old = file.old.as_ref().ok_or_else(|| {
                    input("journal backup has no expected identity")
                })?;
                let previous = checked_path(
                    root,
                    file.previous_path.as_deref().unwrap_or(&file.path),
                )?;
                if !matches_file(&previous, old).await? {
                    return Err(input(
                        "resource recovery backup missing and original file not restored",
                    ));
                }
                if previous != path && path.exists() {
                    if !matches_file(&path, &file.target).await? {
                        return Err(input(
                            "resource file changed during recovery",
                        ));
                    }
                    tokio::fs::remove_file(&path).await?;
                }
            }
        } else if path.exists() {
            if !matches_file(&path, &file.target).await? {
                return Err(input("new resource file changed during recovery"));
            }
            tokio::fs::remove_file(&path).await?;
        }
        let temporary = PathBuf::from(format!("{}.installing", path.display()));
        if temporary.exists() {
            tokio::fs::remove_file(temporary).await?;
        }
    }
    Ok(())
}

pub(crate) async fn execute(
    instance_id: &str,
    plan: &ResourceOperationPlan,
    reporter: Option<crate::install::InstallProgressReporter>,
    state: &crate::State,
) -> crate::Result<()> {
    let _cache_usage = crate::storage::usage::lease().await?;
    if plan.instance_id != instance_id
        || plan.issues.iter().any(|issue| issue.blocking)
    {
        return Err(input("resource plan is not executable"));
    }
    let completed = ResourceOperationPlans::completed_at(
        &state.data,
        plan.id.as_str(),
        plan.revision,
    )
    .await?;
    if completed == Some(1) {
        return Ok(());
    }
    if completed.is_none() {
        return Err(input("durable resource plan not found"));
    }
    let payload = ResourceOperationPlans::payload_at(
        &state.data,
        plan.id.as_str(),
        plan.revision,
    )
    .await?;
    let draft: plans::OperationDraft = serde_json::from_str(&payload)?;
    if draft.plan != *plan {
        return Err(input("task does not match the reviewed resource plan"));
    }
    let cancel = reporter
        .as_ref()
        .map(|reporter| reporter.cancellation_token())
        .unwrap_or_default();
    canceled(&cancel)?;
    clear_staging(plan, state).await?;
    let staging = state.directories.caches_dir().join("resource-staging");
    tokio::fs::create_dir_all(&staging).await?;
    let temporary = tempfile::Builder::new()
        .prefix(&staging_prefix(plan)?)
        .tempdir_in(staging)?;
    let stage_cancel = cancel.child_token();
    let concurrency = std::sync::Arc::new(tokio::sync::Semaphore::new(
        state.download_concurrency().max(1),
    ));
    let pending = FuturesUnordered::new();
    for (index, action) in plan.actions.iter().enumerate() {
        let cancel = &stage_cancel;
        let draft = &draft;
        let temporary = &temporary;
        let reporter = &reporter;
        let concurrency = concurrency.clone();
        let download = async move {
            let _permit = tokio::select! {
                _ = cancel.cancelled() => return Err(input("resource operation canceled")),
                permit = concurrency.acquire_owned() => permit.map_err(|error| input(error.to_string()))?,
            };
            canceled(cancel)?;
            let node = plan
                .nodes
                .iter()
                .find(|node| node.id == action.node_id)
                .ok_or_else(|| input("action has no resource node"))?;
            let kind = action
                .current
                .as_ref()
                .map(|current| current.resource_type)
                .or_else(|| {
                    plan.selections
                        .iter()
                        .find(|selection| selection.id == node.id)
                        .map(|selection| selection.resource_type)
                })
                .unwrap_or_else(|| {
                    if action.target_relative_path.contains("/datapacks/") {
                        ResourceType::DataPack
                    } else if action
                        .target_relative_path
                        .starts_with("resourcepacks/")
                    {
                        ResourceType::ResourcePack
                    } else if action
                        .target_relative_path
                        .starts_with("shaderpacks/")
                    {
                        ResourceType::Shader
                    } else {
                        ResourceType::Mod
                    }
                });
            project_type(kind)?;
            if action.target.reference.release.project.source.as_str()
                != "planet_minecraft"
            {
                provider_ref(&action.target)?;
            }
            if !valid_artifact(&action.target)
                || !plans::safe_relative_path(&action.target_relative_path)
            {
                return Err(input("invalid frozen artifact or destination"));
            }
            if draft.local_files.contains_key(&action.entry_id)
                || draft.import_files.contains_key(&action.node_id)
            {
                let scope =
                    resolve_content_scope(instance_id, None, state).await?;
                let root =
                    instance_content_root(&state.directories, &scope.instance)?;
                let source = if let Some(path) =
                    draft.import_files.get(&action.node_id)
                {
                    let path = PathBuf::from(path);
                    if !path.is_absolute() {
                        return Err(input("invalid local import path"));
                    }
                    path
                } else {
                    checked_path(&root, &draft.local_files[&action.entry_id])?
                };
                if !matches_file(&source, &action.target.integrity).await? {
                    return Err(input(
                        "rollback backup no longer matches reviewed bytes",
                    ));
                }
                let path = temporary.path().join(index.to_string());
                tokio::fs::copy(source, &path).await?;
                if !matches_file(&path, &action.target.integrity).await? {
                    return Err(input("rollback backup changed while copying"));
                }
                let (size, sha1) = fetch::sha1_file_async(&path).await?;
                return Ok::<_, crate::Error>((
                    index,
                    (path, kind, size, sha1, String::new()),
                ));
            }
            let urls = match &action.target.acquisition {
                ArtifactAcquisition::PublicDownload { urls }
                    if !urls.is_empty() =>
                {
                    urls
                }
                _ => {
                    return Err(input("artifact requires a manual download"));
                }
            };
            let path = temporary.path().join(index.to_string());
            let class =
                match action.target.reference.release.project.source.as_str() {
                    "modrinth" => ResourceClass::Modrinth,
                    "curseforge" => ResourceClass::CurseForge,
                    _ => ResourceClass::Other,
                };
            let mut request = DownloadRequest::new(&urls[0], class)
                .with_integrity(integrity(&action.target, kind));
            request.cancellation = Some(cancel.clone());
            request.candidate_urls = urls.iter().skip(1).cloned().collect();
            if let Some(reporter) = reporter.as_ref().cloned() {
                request = request.with_install_tracking(
                    reporter,
                    action.target_relative_path.clone(),
                    action.target.file_name.clone().unwrap_or_default(),
                );
            }
            request.cancellation = Some(cancel.clone());
            fetch::download_to_path(
                request,
                &path,
                &state.download_semaphore,
                None,
            )
            .await?;
            let (size, sha1) = fetch::sha1_file_async(&path).await?;
            Ok::<_, crate::Error>((
                index,
                (path, kind, size, sha1, String::new()),
            ))
        };
        pending.push(download.boxed());
    }
    let staged = pending.try_collect::<Vec<_>>().await;
    let mut staged = match staged {
        Ok(files) => files,
        Err(error) => {
            stage_cancel.cancel();
            return Err(error);
        }
    };
    staged.sort_by_key(|(index, _)| *index);
    let mut downloads: Vec<_> =
        staged.into_iter().map(|(_, file)| file).collect();
    for (action, (path, _, size, _, digest)) in
        plan.actions.iter().zip(&mut downloads)
    {
        if action
            .target
            .integrity
            .size
            .is_some_and(|expected| expected != *size)
        {
            return Err(input(
                "staged file size disagrees with reviewed artifact",
            ));
        }
        canceled(&cancel)?;
        let actual = fetch::compute_file_integrity(
            path,
            &Integrity {
                sha512: Some(String::new()),
                ..Default::default()
            },
        )
        .await?;
        *digest = actual
            .sha512
            .ok_or_else(|| input("staged artifact has no SHA-512"))?;
        *path = crate::state::content_store::publish_verified(
            path,
            &state.directories,
            digest,
        )
        .await?;
        crate::state::content_store::record_published(
            &state.pool,
            digest,
            *size,
        )
        .await?;
    }
    let mut retained_parents = HashMap::new();
    for (action, (path, kind, _, _, _)) in plan
        .actions
        .iter()
        .zip(&downloads)
        .filter(|_| draft.resolve_dependencies)
    {
        let service = state.resources().await?;
        let node = plan
            .nodes
            .iter()
            .find(|node| node.id == action.node_id)
            .ok_or_else(|| input("action has no resource node"))?;
        let selection = ResourceSelection {
            id: node.id.clone(),
            resource: node.resource.clone(),
            release: ReleaseSelection::Artifact {
                reference: action.target.reference.clone(),
            },
            resource_type: *kind,
            scope: node.scope.clone(),
            entry_id: Some(action.entry_id.clone()),
        };
        let requirements = service
            .requirements_for(&selection, &draft.retained_requirements)
            .await
            .map_err(|error| input(error.to_string()))?;
        let metadata = if requirements
            .iter()
            .any(|requirement| requirement.local_mod_id.is_some())
        {
            let bytes = bytes::Bytes::from(tokio::fs::read(path).await?);
            crate::mod_metadata::extract_mod_metadata(&bytes)
        } else {
            None
        };
        for requirement in requirements {
            if let Some(id) = &requirement.local_mod_id
                && let Some(metadata) = &metadata
                && (&metadata.mod_id != id
                    || !requirement.version_constraint.as_ref().is_none_or(
                        |constraint| {
                            metadata.version.as_ref().is_none_or(|version| {
                                plans::requirements::mod_version_matches(
                                    constraint, version,
                                )
                            })
                        },
                    ))
            {
                return Err(input(format!(
                    "updated mod does not satisfy {}'s requirement for {id}",
                    requirement.parent.relative_path
                )));
            }
            retained_parents.insert(
                requirement.parent.entry_id.clone(),
                requirement.parent,
            );
        }
    }
    canceled(&cancel)?;
    let write_started = std::time::Instant::now();
    tracing::info!(
        plan_id = plan.id.as_str(),
        instance_id,
        actions = plan.actions.len(),
        "Resource execution waiting for database writer"
    );
    let _writer = state.acquire_install_db_permit().await?;
    tracing::info!(
        plan_id = plan.id.as_str(),
        instance_id,
        elapsed_ms = write_started.elapsed().as_millis() as u64,
        "Resource execution waiting for instance lock"
    );
    let _lock = state.lock_instance_content_exclusive(instance_id).await;
    tracing::info!(
        plan_id = plan.id.as_str(),
        instance_id,
        elapsed_ms = write_started.elapsed().as_millis() as u64,
        "Resource execution acquired write access"
    );
    let scope = resolve_content_scope(instance_id, None, state).await?;
    if scope.instance.install_stage
        != crate::state::InstanceInstallStage::Installed
    {
        return Err(input("instance is not ready for resource changes"));
    }
    let root = instance_content_root(&state.directories, &scope.instance)?;
    if ResourceOperationPlans::completed(&state.data, plan.id.as_str()).await?
        == 1
    {
        return Ok(());
    }
    let previous_journal =
        ResourceExecutionJournals::for_plan(&state.data, plan.id.as_str())
            .await?;
    if let Some(journal) = previous_journal {
        let _critical = crate::install::critical_section::enter();
        recover_files(
            &root,
            &serde_json::from_str::<Vec<JournalFile>>(&journal)?,
        )
        .await?;
    }
    let current_revision = plans::instance_revision(&state.pool, instance_id)
        .await
        .map_err(|error| input(error.to_string()))?;
    if current_revision != plan.instance_revision {
        return Err(input("instance changed since resource review"));
    }
    let mut executable = draft.clone();
    if executable.skip_unwritable_files {
        super::write_access::review_at_root(&mut executable, &root)
            .map_err(|error| input(error.to_string()))?;
        if executable.plan.issues.iter().any(|issue| issue.blocking) {
            return Err(input(
                "resource files changed since review; refresh the preview",
            ));
        }
        let retained: std::collections::HashSet<_> = executable
            .plan
            .actions
            .iter()
            .map(|action| &action.entry_id)
            .collect();
        downloads = plan
            .actions
            .iter()
            .zip(downloads)
            .filter_map(|(action, file)| {
                retained.contains(&action.entry_id).then_some(file)
            })
            .collect();
    }
    let draft = &executable;
    let plan = &executable.plan;
    if let Some(reporter) = &reporter {
        let skipped = plan
            .issues
            .iter()
            .filter(|issue| issue.code == ResourceIssueCode::FileWriteSkipped)
            .map(|issue| {
                crate::install::InstallJobEventKind::ContentFileSkipped {
                    path: issue.message.clone(),
                    reason: "File cannot be written".into(),
                    project_id: None,
                    version_id: None,
                    manual_url: None,
                }
            })
            .collect::<Vec<_>>();
        if !skipped.is_empty() {
            reporter.record_events(skipped).await?;
        }
    }
    if plan.actions.is_empty() {
        ResourceOperationPlans::mark_completed(
            &state.data,
            plan.id.as_str(),
            plan.revision,
        )
        .await?;
        return Ok(());
    }
    for parent in retained_parents.values() {
        if !matches_file(
            &checked_path(&root, &parent.relative_path)?,
            &parent.integrity,
        )
        .await?
        {
            return Err(input(
                "installed dependent changed since resource review",
            ));
        }
    }
    let mut journal = Vec::new();
    for (index, action) in plan.actions.iter().enumerate() {
        let destination = checked_path(&root, &action.target_relative_path)?;
        if let Some(current) = &action.current
            && !current.missing
        {
            let previous = checked_path(&root, &current.relative_path)?;
            if !matches_file(&previous, &current.integrity).await? {
                return Err(input("installed artifact changed since review"));
            }
            if previous != destination && destination.exists() {
                return Err(input("resource destination already exists"));
            }
            let parent = action
                .target_relative_path
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .unwrap_or_default();
            let backup =
                format!("{parent}/.resource-{}-{index}.old", plan.id.as_str());
            if checked_path(&root, &backup)?.exists() {
                return Err(input("resource backup path is occupied"));
            }
            journal.push(JournalFile {
                path: action.target_relative_path.clone(),
                previous_path: Some(current.relative_path.clone()),
                backup: Some(backup),
                old: Some(current.integrity.clone()),
                target: action.target.integrity.clone(),
            });
        } else {
            if destination.exists() {
                return Err(input("resource destination already exists"));
            }
            journal.push(JournalFile {
                path: action.target_relative_path.clone(),
                previous_path: None,
                backup: None,
                old: None,
                target: action.target.integrity.clone(),
            });
        }
    }
    let installed = plans::installed(&state.pool, instance_id)
        .await
        .map_err(|error| input(error.to_string()))?;
    for node in plan
        .nodes
        .iter()
        .filter(|node| node.state == DependencyNodeState::Satisfied)
    {
        for entry_id in &node.installed_entries {
            let item = installed
                .iter()
                .find(|item| &item.entry_id == entry_id)
                .ok_or_else(|| input("satisfied dependency disappeared"))?;
            if let Some(action) = plan
                .actions
                .iter()
                .find(|action| &action.entry_id == entry_id)
                && node.artifact.is_none()
                && (action
                    .current
                    .as_ref()
                    .is_some_and(|current| !current.enabled)
                    || action.target_relative_path.rsplit('/').next()
                        != item.relative_path.rsplit('/').next())
            {
                return Err(input(
                    "updated local file no longer satisfies its filename dependency",
                ));
            }
            if !item.enabled
                || !matches_file(
                    &checked_path(&root, &item.relative_path)?,
                    &item.integrity,
                )
                .await?
            {
                return Err(input("satisfied dependency changed"));
            }
        }
    }
    canceled(&cancel)?;
    ResourceExecutionJournals::save(
        &state.data,
        plan.id.as_str(),
        &serde_json::to_string(&journal)?,
    )
    .await?;
    let _critical = crate::install::critical_section::enter();
    tracing::info!(
        plan_id = plan.id.as_str(),
        instance_id,
        "Resource execution publishing verified files"
    );
    let mut installed_fingerprints = Vec::new();
    let result: crate::Result<()> = async {
        for ((file, (source, _, size, sha1, _)), _) in
            journal.iter().zip(&downloads).zip(&plan.actions)
        {
            let destination = checked_path(&root, &file.path)?;
            if let Some(backup) = &file.backup {
                tokio::fs::rename(
                    checked_path(
                        &root,
                        file.previous_path.as_deref().unwrap_or(&file.path),
                    )?,
                    checked_path(&root, backup)?,
                )
                .await?;
            }
            let (_, modified) = materialize_verified_project_download_copy(
                source,
                &destination,
                *size,
                sha1,
            )
            .await?;
            installed_fingerprints.push((
                file.path.clone(),
                *size,
                sha1.clone(),
                modified,
            ));
        }
        persist_plan(
            &state.pool,
            state.cache_pool(),
            &scope.content_set_id,
            plan,
            &downloads,
            &journal,
            draft,
        )
        .await
    }
    .await;
    if let Err(error) = result {
        match recover_files(&root, &journal).await {
            Ok(()) => {
                ResourceExecutionJournals::delete(
                    &state.data,
                    plan.id.as_str(),
                )
                .await?;
            }
            Err(recovery) => {
                return Err(input(format!(
                    "{error}; resource recovery requires retry: {recovery}"
                )));
            }
        }
        return Err(error);
    }
    for (path, size, sha1, modified) in installed_fingerprints {
        if let Err(error) = state
            .file_fingerprints
            .seed_verified(
                state,
                &plan.instance_id,
                &root,
                &path,
                ArtifactIntegrity {
                    size: Some(size),
                    hashes: vec![ContentHash {
                        algorithm: HashAlgorithm::Sha1,
                        value: sha1,
                    }],
                },
                modified,
            )
            .await
        {
            tracing::warn!(%error, "Could not save verified installed fingerprint");
        }
    }
    drop(_critical);
    drop(_lock);
    drop(_writer);
    tracing::info!(
        plan_id = plan.id.as_str(),
        instance_id,
        elapsed_ms = write_started.elapsed().as_millis() as u64,
        "Resource execution committed files and database"
    );
    if let Some(reporter) = reporter {
        reporter
            .update(
                crate::install::InstallPhaseId::DownloadingContent,
                Some(crate::install::InstallProgress {
                    current: downloads.len() as u64,
                    total: downloads.len() as u64,
                    secondary: None,
                }),
                crate::install::InstallPhaseDetails::Empty,
            )
            .await?;
    }
    Ok(())
}

pub(crate) async fn recover_plan(
    plan: &ResourceOperationPlan,
    state: &crate::State,
) -> crate::Result<()> {
    let _writer = state.acquire_install_db_permit().await?;
    let _lock = state
        .lock_instance_content_exclusive(&plan.instance_id)
        .await;
    let payload = ResourceExecutionJournals::pending_payload(
        &state.data,
        plan.id.as_str(),
    )
    .await?;
    let _critical = crate::install::critical_section::enter();
    if let Some(payload) = payload {
        let scope =
            resolve_content_scope(&plan.instance_id, None, state).await?;
        let root = instance_content_root(&state.directories, &scope.instance)?;
        recover_files(
            &root,
            &serde_json::from_str::<Vec<JournalFile>>(&payload)?,
        )
        .await?;
        ResourceExecutionJournals::delete(&state.data, plan.id.as_str())
            .await?;
    }
    clear_staging(plan, state).await
}

async fn persist_plan(
    pool: &SqlitePool,
    cache_pool: &SqlitePool,
    set_id: &str,
    plan: &ResourceOperationPlan,
    downloads: &[(PathBuf, ResourceType, u64, String, String)],
    journal: &[JournalFile],
    draft: &plans::OperationDraft,
) -> crate::Result<()> {
    let old_dependencies =
        content_entries::get_content_dependency_edges(set_id, pool).await?;
    let old_members = pack_members::get_pack_members(set_id, pool).await?;
    let mut prior = HashMap::new();
    for action in &plan.actions {
        let id = action.entry_id.as_str();
        let refs =
            content_entries::get_content_provider_refs_with_origin(id, pool)
                .await?;
        let key = id.to_owned();
        let graph = ResourceEntryGraphs::get(&Handle::new(pool.clone()), &key)
            .await?
            .map(|record| (record.nodes, record.edges));
        let entry = content_entries::get_content_entry_by_id(id, pool).await?;
        prior.insert(action.entry_id.clone(), (refs, graph, entry));
    }
    let instance_path = crate::state::instances::adapters::sqlite::instance_rows::get_instance_path_by_id(
        &plan.instance_id,
        pool,
    )
    .await?
    .ok_or_else(|| input("instance disappeared before the plan was stored"))?;
    let mut mod_metadata = HashMap::new();
    for (action, (source_path, kind, _, _, _)) in
        plan.actions.iter().zip(downloads)
    {
        if *kind == ResourceType::Mod {
            let bytes = bytes::Bytes::from(tokio::fs::read(source_path).await?);
            let metadata = tokio::task::spawn_blocking(move || {
                crate::mod_metadata::extract_mod_metadata(&bytes)
                    .and_then(|metadata| serde_json::to_string(&metadata).ok())
            })
            .await
            .map_err(|error| input(error.to_string()))?;
            mod_metadata.insert(action.entry_id.clone(), metadata);
        }
    }
    let mut tx = pool.begin().await?;
    let mut node_entries: HashMap<SelectionId, Vec<InstalledEntryId>> = plan
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.installed_entries.clone()))
        .collect();
    let now = chrono::Utc::now();
    let mut cache_entries = Vec::new();
    let restore: Option<serde_json::Value> =
        if let Some(id) = &draft.restore_history {
            let payload =
                ResourceHistory::backup_payload(&mut tx, id.as_str()).await?;
            Some(serde_json::from_str(&payload)?)
        } else {
            None
        };
    for ((action, (_, kind, size, sha1, digest)), file) in
        plan.actions.iter().zip(downloads).zip(journal)
    {
        let id = action.entry_id.as_str();
        let (old_refs, old_graph, existing) = prior
            .remove(&action.entry_id)
            .ok_or_else(|| input("entry snapshot missing"))?;
        let old_entry = existing.clone();
        let previous_artifact: Option<String> =
            super::installed::installed_artifact_payload(id, &mut tx).await?;
        let file_name = action
            .target_relative_path
            .rsplit('/')
            .next()
            .unwrap_or_default();
        if action
            .target
            .integrity
            .size
            .is_some_and(|expected| expected != *size)
        {
            return Err(input(
                "artifact size disagrees with the reviewed snapshot",
            ));
        }
        let local_mod_data = mod_metadata.remove(&action.entry_id).flatten();
        if let Some(current) = &action.current {
            files::rename_entry_file_in_transaction(
                &plan.instance_id,
                id,
                &current.relative_path,
                &action.target_relative_path,
                file_name,
                &mut tx,
            )
            .await?;
        }
        let disk_file = files::upsert_instance_file_from_parts_in_transaction(
            files::UpsertInstanceFile {
                instance_id: &plan.instance_id,
                relative_path: &action.target_relative_path,
                file_name,
                enabled: action
                    .current
                    .as_ref()
                    .is_none_or(|current| current.enabled),
                sha1,
                size: *size,
                missing: false,
                local_mod_data: local_mod_data.as_deref(),
                icon_path: None,
            },
            &mut tx,
        )
        .await?;
        let cache =
            crate::state::CacheValue::FileHash(crate::state::CachedFileHash {
                path: format!(
                    "{instance_path}/{}",
                    action.target_relative_path
                ),
                size: *size,
                hash: sha1.clone(),
                project_type: Some(project_type(*kind)?),
                project_id: (action.target.reference.release.project.source
                    == SourceId::modrinth())
                .then(|| {
                    action
                        .target
                        .reference
                        .release
                        .project
                        .project_id
                        .as_str()
                        .into()
                }),
                version_id: (action.target.reference.release.project.source
                    == SourceId::modrinth())
                .then(|| {
                    action.target.reference.release.release_id.as_str().into()
                }),
            })
            .get_entry();
        cache_entries.push(cache);
        files::set_local_mod_data_in_transaction(
            &disk_file.id,
            local_mod_data.as_deref(),
            &mut tx,
        )
        .await?;
        let old_path = action
            .current
            .as_ref()
            .map(|current| current.relative_path.as_str())
            .unwrap_or(&action.target_relative_path);
        let old_digest = content_store_files::get_instance_file_digest(
            &plan.instance_id,
            old_path,
            &mut *tx,
        )
        .await?;
        if old_path != action.target_relative_path {
            content_store_files::remove_instance_files(
                &mut tx,
                &plan.instance_id,
                &[old_path],
            )
            .await?;
        }
        content_store_files::record_instance_file(
            &mut tx,
            &plan.instance_id,
            &action.target_relative_path,
            digest,
            now.timestamp(),
            now.timestamp(),
        )
        .await?;
        if let Some(existing) = existing {
            if existing.file_id.as_deref() != Some(&disk_file.id)
                || existing.content_set_id != set_id
            {
                return Err(input("resource entry ownership changed"));
            }
            content_entries::set_entry_enabled_in_transaction(
                id,
                disk_file.enabled,
                now.timestamp(),
                &mut tx,
            )
            .await?;
        } else {
            let entry =
                content_entries::upsert_content_entry_from_parts_in_transaction(
                    content_entries::UpsertContentEntry {
                        instance_id: &plan.instance_id,
                        content_set_id: set_id,
                        file_id: Some(&disk_file.id),
                        project_type: project_type(*kind)?,
                        source_kind: ContentSourceKind::Local,
                        ownership_kind: ContentOwnershipKind::UserAdded,
                        auto_dependency: !plan
                            .active_roots
                            .contains(&action.node_id),
                        server_requirement: ContentRequirement::Unknown,
                        client_requirement: ContentRequirement::Unknown,
                        enabled: disk_file.enabled,
                    },
                    &mut tx,
                )
                .await?;
            if action.current.is_some() {
                return Err(input("original resource entry disappeared"));
            }
            content_entries::rewrite_entry_id_in_transaction(
                &entry.id, id, &mut tx,
            )
            .await?;
        }
        content_entries::delete_provider_refs_for_entry_in_transaction(
            id, &mut tx,
        )
        .await?;
        if action.target.reference.release.project.source.as_str()
            != "planet_minecraft"
        {
            let reference = provider_ref(&action.target)?;
            content_entries::upsert_content_provider_ref_in_transaction(
                id, &reference, true, &mut tx,
            )
            .await?;
        }
        if let Some(restore) = &restore {
            let previous_refs: Vec<(ContentProviderRef, bool)> =
                serde_json::from_value(restore["restoreProviderRefs"].clone())?;
            content_entries::delete_provider_refs_for_entry_in_transaction(
                id, &mut tx,
            )
            .await?;
            for (previous, origin) in previous_refs {
                content_entries::upsert_content_provider_ref_in_transaction(
                    id, &previous, origin, &mut tx,
                )
                .await?;
            }
            let metadata: crate::state::ContentEntry = serde_json::from_value(
                restore["restoreEntryMetadata"].clone(),
            )?;
            let unlinked = resource_pack_unlinks::marked(
                &mut tx,
                &plan.instance_id,
                set_id,
            )
            .await?;
            let ownership = if unlinked
                && metadata.ownership_kind == ContentOwnershipKind::PackManaged
            {
                ContentOwnershipKind::UserAdded
            } else {
                metadata.ownership_kind
            };
            let source = if unlinked {
                ContentSourceKind::Local
            } else {
                metadata.source_kind
            };
            content_entries::set_entry_provenance_in_transaction(
                id,
                ownership,
                source,
                metadata.auto_dependency,
                metadata.server_requirement,
                metadata.client_requirement,
                &mut tx,
            )
            .await?;
        }
        let mut verified_artifact = action.target.clone();
        verified_artifact.integrity.size = Some(*size);
        if !verified_artifact
            .integrity
            .hashes
            .iter()
            .any(|hash| hash.algorithm == HashAlgorithm::Sha1)
        {
            verified_artifact.integrity.hashes.push(ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1.clone(),
            });
        }
        let node = plan
            .nodes
            .iter()
            .find(|node| node.id == action.node_id)
            .ok_or_else(|| input("node missing"))?;
        let record = super::installed::InstalledArtifactRecord {
            artifact: verified_artifact,
            declared_compatibility: node.declared_compatibility.clone(),
        };
        super::installed::set_installed_artifact(
            id,
            &record,
            &node.scope,
            &mut tx,
        )
        .await?;
        content_entries::delete_update_check_for_entry_in_transaction(
            id, &mut tx,
        )
        .await?;
        pack_members::mark_entry_version_override_in_transaction(id, &mut tx)
            .await?;
        node_entries
            .entry(action.node_id.clone())
            .or_default()
            .push(action.entry_id.clone());
        if let Some(current) = &action.current {
            let mut target = current.clone();
            target.artifact = Some(action.target.reference.clone());
            target.known_artifacts = vec![action.target.reference.clone()];
            target.integrity = action.target.integrity.clone();
            target.integrity.size = Some(*size);
            if !target
                .integrity
                .hashes
                .iter()
                .any(|hash| hash.algorithm == HashAlgorithm::Sha1)
            {
                target.integrity.hashes.push(ContentHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: sha1.clone(),
                });
            }
            target.original_file_name = action.target.file_name.clone();
            target.relative_path = action.target_relative_path.clone();
            target.missing = false;
            let mut history = history::history_entry(
                &plan.instance_id,
                current,
                target,
                plan,
                file.backup.clone(),
            );
            if let Some((nodes, edges)) = old_graph {
                history.previous_nodes = serde_json::from_str(&nodes)?;
                history.previous_dependencies = serde_json::from_str(&edges)?;
            } else {
                history.previous_nodes.clear();
                history.previous_dependencies.clear();
            }
            if let Some(old_digest) = old_digest
                && file.backup.is_some()
            {
                retained_refs::retain(
                    &mut *tx,
                    &old_digest,
                    &format!("resource-history:{}", history.id.as_str()),
                    now.timestamp(),
                )
                .await?;
            }
            let mut payload = serde_json::to_value(&history)?;
            payload["restoreProviderRefs"] = serde_json::to_value(old_refs)?;
            payload["restoreEntryMetadata"] = serde_json::to_value(old_entry)?;
            payload["restorePackMember"] =
                serde_json::to_value(old_members.iter().find(|member| {
                    member.content_entry_id.as_deref() == Some(id)
                }))?;
            payload["previousArtifact"] = previous_artifact
                .map(|value| serde_json::from_str::<serde_json::Value>(&value))
                .transpose()?
                .unwrap_or(serde_json::Value::Null);
            payload["restoreDependencies"] = serde_json::to_value(
                old_dependencies
                    .iter()
                    .filter(|edge| edge.parent_entry_id == id)
                    .collect::<Vec<_>>(),
            )?;
            ResourceHistory::insert(
                &mut tx,
                history.id.as_str(),
                &ResourceHistoryRecord {
                    instance_id: plan.instance_id.clone(),
                    entry_id: id.to_string(),
                    recorded_at: now.timestamp(),
                    payload: payload.to_string(),
                    backup_relative_path: file.backup.clone(),
                    backup_size: current.integrity.size.map(|size| size as i64),
                    backup_sha1: current
                        .integrity
                        .hashes
                        .iter()
                        .find(|hash| hash.algorithm == HashAlgorithm::Sha1)
                        .map(|hash| hash.value.clone()),
                    backup_available: history.backup_available,
                },
            )
            .await?;
        }
        let (nodes, edges) = entry_graph(plan, &action.node_id);
        let graph = EntryGraphRecord {
            nodes: serde_json::to_string(&nodes)?,
            edges: serde_json::to_string(&edges)?,
        };
        let key = id.to_owned();
        ResourceEntryGraphs::write(&mut tx, &key, Some(&graph)).await?;
    }
    for action in &plan.actions {
        let mut edges = Vec::new();
        for edge in plan.edges.iter().filter(|edge| {
            edge.parent == action.node_id
                && edge.kind == ResourceDependencyKind::Required
        }) {
            let child_node =
                plan.nodes.iter().find(|node| node.id == edge.child);
            if let (Some(child_node), Some(children)) =
                (child_node, node_entries.get(&edge.child))
                && let Some(artifact) = &child_node.artifact
                && artifact.reference.release.project.source.as_str()
                    != "planet_minecraft"
                && action.target.reference.release.project.source.as_str()
                    != "planet_minecraft"
            {
                let child_ref = provider_ref(artifact)?;
                let parent_ref = provider_ref(&action.target)?;
                for child in children {
                    edges.push(crate::state::ContentDependencyEdge { id: uuid::Uuid::new_v4().to_string(), content_set_id: set_id.into(), parent_entry_id: action.entry_id.as_str().into(), child_entry_id: child.as_str().into(), evidence_provider: parent_ref.provider(), parent_provider: parent_ref.provider(), child_provider: child_ref.provider(), dependency_kind: crate::state::instances::ContentDependencyKind::Required, parent_project_id: parent_ref.database_project_id(), parent_release_id: parent_ref.database_release_id().unwrap_or_default(), child_project_id: child_ref.database_project_id(), child_release_id: child_ref.database_release_id().unwrap_or_default(), created_at: now, modified_at: now });
                }
            }
        }
        if let Some(restore) = &restore {
            edges =
                serde_json::from_value(restore["restoreDependencies"].clone())?;
        }
        content_entries::replace_content_dependency_edges_for_parent(
            set_id,
            action.entry_id.as_str(),
            &edges,
            &mut tx,
        )
        .await?;
    }
    if let (Some(id), Some(mut payload)) = (&draft.restore_history, restore) {
        payload["backupAvailable"] = serde_json::Value::Bool(false);
        payload["backupRetired"] = serde_json::Value::Bool(true);
        let previous: ResourceHistoryEntry =
            serde_json::from_value(payload.clone())?;
        if let Some(member) = payload
            .get("restorePackMember")
            .filter(|value| !value.is_null())
        {
            let member: crate::state::PackMember =
                serde_json::from_value(member.clone())?;
            let unlinked = resource_pack_unlinks::marked(
                &mut tx,
                &plan.instance_id,
                set_id,
            )
            .await?;
            if !unlinked {
                pack_members::upsert_pack_member_in_transaction(
                    &member, &mut tx,
                )
                .await?;
            }
        }
        ResourceEntryGraphs::restore_in(
            &mut tx,
            previous.entry_id.as_str(),
            &EntryGraphRecord {
                nodes: serde_json::to_string(&previous.previous_nodes)?,
                edges: serde_json::to_string(&previous.previous_dependencies)?,
            },
        )
        .await?;
        let payload = payload.to_string();
        ResourceHistory::retire_with_payload_in(&mut tx, id.as_str(), &payload)
            .await?;
    }
    content_sets::bump_content_set_revision_in_transaction(set_id, &mut tx)
        .await?;
    ResourceOperationPlans::mark_completed_in(
        &mut tx,
        plan.id.as_str(),
        plan.revision,
    )
    .await?;
    ResourceExecutionJournals::delete_in(&mut tx, plan.id.as_str()).await?;
    // The hash cache lives in the derived database. Dropping the entries the
    // commit is about to replace keeps a run that stops before the write below
    // from trusting metadata of the file that was just replaced.
    crate::state::CachedEntry::forget_best_effort(&cache_entries, cache_pool)
        .await;
    tx.commit().await?;
    crate::state::CachedEntry::store_best_effort(&cache_entries, cache_pool)
        .await;
    Ok(())
}

#[cfg(test)]
#[path = "execution_tests.rs"]
pub(crate) mod tests;
