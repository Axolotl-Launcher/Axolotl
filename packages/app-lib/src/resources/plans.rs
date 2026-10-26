pub mod analysis;
pub(crate) mod artifact_metadata;
pub(crate) mod requirements;

use futures::StreamExt;
use requirements::RetainedRequirement;

use super::{ipc::ResourceTarget, providers::*, *};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceOperationProgress {
    pub plan_id: PlanId,
    pub revision: u32,
    pub node: ResourceDependencyNode,
    pub metadata: Option<SourceProjectMetadata>,
}

pub type OperationProgress<'a> =
    Option<&'a (dyn Fn(ResourceOperationProgress) + Send + Sync)>;

fn report_node(
    plan: &ResourceOperationPlan,
    metadata: Option<SourceProjectMetadata>,
    progress: OperationProgress<'_>,
) {
    if let (Some(progress), Some(node)) = (progress, plan.nodes.last()) {
        progress(ResourceOperationProgress {
            plan_id: plan.id.clone(),
            revision: plan.revision,
            node: node.clone(),
            metadata,
        });
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct OperationDraft {
    #[serde(default)]
    pub local_metadata:
        HashMap<InstalledEntryId, crate::mod_metadata::LocalModMetadata>,
    #[serde(default)]
    pub retained_requirements: Vec<RetainedRequirement>,
    #[serde(default = "resolve_dependencies_default")]
    pub resolve_dependencies: bool,
    #[serde(default)]
    pub skip_unwritable_files: bool,
    #[serde(default)]
    pub naming: super::naming::NamingPreferences,
    pub plan: ResourceOperationPlan,
    pub install_dependencies: bool,
    pub candidates: Vec<ResourceSelection>,
    #[serde(default)]
    pub local_files: HashMap<InstalledEntryId, String>,
    #[serde(default)]
    pub import_files: HashMap<SelectionId, String>,
    #[serde(default)]
    pub restore_history: Option<HistoryId>,
}

fn resolve_dependencies_default() -> bool {
    true
}

pub(crate) fn db(error: sqlx::Error) -> ResourceError {
    ResourceError::Database(error.to_string())
}
fn invalid(message: impl Into<String>) -> ResourceError {
    ResourceError::InvalidRequest(message.into())
}
fn issue(
    plan: &mut ResourceOperationPlan,
    code: ResourceIssueCode,
    node: &SelectionId,
    blocking: bool,
    message: impl Into<String>,
) {
    plan.issues.push(ResourcePlanIssue {
        code,
        selection_id: Some(node.clone()),
        entry_id: None,
        blocking,
        message: message.into(),
    });
}

fn dependency_analysis_issue(
    plan: &mut ResourceOperationPlan,
    node: &SelectionId,
    message: impl Into<String>,
) {
    let allow_install = plan.intent == ResourceOperationIntent::Install;
    issue(
        plan,
        ResourceIssueCode::DependencyAnalysisFailed,
        node,
        !allow_install,
        message,
    );
}

fn dependency_source_failed(error: &ResourceError) -> bool {
    matches!(
        error,
        ResourceError::Network { .. }
            | ResourceError::Unavailable
            | ResourceError::InvalidResponse(_)
            | ResourceError::Unsupported(_)
    )
}

fn selection_matches_artifact(
    selection: &ReleaseSelection,
    artifact: Option<&ResourceArtifact>,
) -> bool {
    match selection {
        ReleaseSelection::Automatic => true,
        ReleaseSelection::Release { reference } => artifact
            .is_some_and(|artifact| &artifact.reference.release == reference),
        ReleaseSelection::Artifact { reference } => {
            artifact.is_some_and(|artifact| &artifact.reference == reference)
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct SelectionKey {
    resource: ResourceRef,
    scope: InstallScope,
    kind: ResourceType,
    entry: Option<InstalledEntryId>,
}

fn selection_key(selection: &ResourceSelection) -> SelectionKey {
    SelectionKey {
        resource: selection.resource.clone(),
        scope: selection.scope.clone(),
        kind: selection.resource_type,
        entry: None,
    }
}

pub fn validate_scope(
    scope: &InstallScope,
    kind: ResourceType,
) -> ResourceResult<()> {
    match (scope, kind) {
        (InstallScope::World { relative_path }, ResourceType::DataPack)
            if safe_relative_path(relative_path)
                && relative_path.starts_with("saves/")
                && relative_path.split('/').count() == 2 =>
        {
            Ok(())
        }
        (
            InstallScope::Instance,
            ResourceType::Mod
            | ResourceType::Plugin
            | ResourceType::Shader
            | ResourceType::ResourcePack,
        ) => Ok(()),
        _ => Err(invalid(
            "this content type requires its supported installation scope",
        )),
    }
}
fn dependency_scope(
    types: &[ResourceType],
    parent: &ResourceSelection,
) -> ResourceResult<(ResourceType, InstallScope)> {
    let installable: Vec<_> = types
        .iter()
        .copied()
        .filter(|kind| {
            matches!(
                kind,
                ResourceType::Mod
                    | ResourceType::Plugin
                    | ResourceType::ResourcePack
                    | ResourceType::Shader
                    | ResourceType::DataPack
            )
        })
        .collect();
    let kind = if installable.contains(&parent.resource_type) {
        parent.resource_type
    } else if installable.len() == 1 {
        installable[0]
    } else {
        return Err(invalid(
            "dependency has no unambiguous installable content type",
        ));
    };
    let scope = if kind == ResourceType::DataPack {
        match &parent.scope {
            InstallScope::World { .. } => parent.scope.clone(),
            _ => {
                return Err(invalid(
                    "a data pack dependency requires an explicit world target",
                ));
            }
        }
    } else {
        InstallScope::Instance
    };
    validate_scope(&scope, kind)?;
    Ok((kind, scope))
}

pub fn safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && !path.chars().any(char::is_control)
        && path.split('/').all(|part| {
            !part.is_empty()
                && !matches!(part, "." | "..")
                && !part.ends_with(['.', ' '])
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}
pub fn artifact_path(
    scope: &InstallScope,
    kind: ResourceType,
    artifact: &ResourceArtifact,
) -> ResourceResult<String> {
    validate_scope(scope, kind)?;
    let file = artifact
        .file_name
        .as_ref()
        .ok_or_else(|| invalid("artifact has no filename"))?;
    let file = crate::util::io::sanitize_download_file_name(file)
        .map_err(|_| invalid("invalid artifact filename"))?;
    let base = match scope {
        InstallScope::World { relative_path } => {
            format!("{relative_path}/datapacks")
        }
        InstallScope::Instance => match kind {
            ResourceType::Mod => "mods",
            ResourceType::Plugin => "plugins",
            ResourceType::Shader => "shaderpacks",
            ResourceType::ResourcePack => "resourcepacks",
            _ => return Err(invalid("unsupported content scope")),
        }
        .into(),
    };
    let allowed = if matches!(kind, ResourceType::Mod | ResourceType::Plugin) {
        file.to_ascii_lowercase().ends_with(".jar")
    } else {
        file.to_ascii_lowercase().ends_with(".zip")
    };
    if !allowed {
        return Err(invalid(
            "artifact file type is not installable in this scope",
        ));
    }
    Ok(format!("{base}/{file}"))
}

pub(crate) async fn instance_revision(
    pool: &SqlitePool,
    instance_id: &str,
) -> ResourceResult<u64> {
    let value: Option<i64> = sqlx::query_scalar("SELECT content.revision FROM instances instance JOIN instance_content_sets content ON content.id = instance.applied_content_set_id WHERE instance.id = ?").bind(instance_id).fetch_optional(pool).await.map_err(db)?;
    value
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| invalid("instance has no applied content set"))
}

pub(crate) async fn installed(
    pool: &SqlitePool,
    instance_id: &str,
) -> ResourceResult<Vec<InstalledResource>> {
    let rows = sqlx::query("SELECT entry.id, entry.project_type, entry.ownership_kind, (SELECT member.id FROM instance_pack_members member WHERE member.content_entry_id = entry.id LIMIT 1) AS member_id, file.relative_path, file.enabled, file.missing, file.sha1, file.size, exact.artifact, exact.scope FROM instance_content_entries entry JOIN instance_content_sets content ON content.id = entry.content_set_id JOIN instance_files file ON file.id = entry.file_id LEFT JOIN resource_installed_artifacts exact ON exact.entry_id = entry.id WHERE entry.instance_id = ? AND content.id = (SELECT applied_content_set_id FROM instances WHERE id = entry.instance_id)").bind(instance_id).fetch_all(pool).await.map_err(db)?;
    let mut result = vec![];
    let all_links = sqlx::query("SELECT link.* FROM resource_entry_links link JOIN instance_content_entries entry ON entry.id = link.entry_id WHERE entry.instance_id = ? ORDER BY link.is_origin DESC").bind(instance_id).fetch_all(pool).await.map_err(db)?;
    let mut links_by_entry: HashMap<String, Vec<sqlx::sqlite::SqliteRow>> =
        HashMap::new();
    for link in all_links {
        links_by_entry
            .entry(link.get("entry_id"))
            .or_default()
            .push(link);
    }
    for row in rows {
        let entry_id: String = row.get("id");
        let kind: String = row.get("project_type");
        let resource_type = match kind.as_str() {
            "mod" => ResourceType::Mod,
            "plugin" => ResourceType::Plugin,
            "shader" | "shaderpack" => ResourceType::Shader,
            "resourcepack" => ResourceType::ResourcePack,
            "datapack" => ResourceType::DataPack,
            _ => continue,
        };
        let path: String = row.get("relative_path");
        let resource_type = if resource_type == ResourceType::Mod
            && path.starts_with("plugins/")
        {
            ResourceType::Plugin
        } else {
            resource_type
        };
        let sha1: String = row.get("sha1");
        let size: i64 = row.get("size");
        let local_integrity = ArtifactIntegrity {
            size: u64::try_from(size).ok(),
            hashes: vec![ContentHash {
                algorithm: HashAlgorithm::Sha1,
                value: sha1,
            }],
        };
        let exact = row
            .get::<Option<String>, _>("artifact")
            .and_then(|value| {
                serde_json::from_str::<ResourceArtifact>(&value).ok()
            })
            .filter(|artifact| {
                identity::exact_integrity(&local_integrity, &artifact.integrity)
            });
        let scope = row
            .get::<Option<String>, _>("scope")
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or_else(|| {
                if resource_type == ResourceType::DataPack
                    && path.starts_with("saves/")
                {
                    InstallScope::World {
                        relative_path: path
                            .split('/')
                            .take(2)
                            .collect::<Vec<_>>()
                            .join("/"),
                    }
                } else {
                    InstallScope::Instance
                }
            });
        let links = links_by_entry.remove(&entry_id).unwrap_or_default();
        let mut resource_id = links.first().and_then(|row| {
            ResourceId::new(row.get::<String, _>("resource_id")).ok()
        });
        if let Some(artifact) = &exact {
            resource_id =
                crate::state::resources::ResourceStore::new(pool.clone())
                    .lookup(&artifact.reference.release.project)
                    .await?
                    .or(resource_id);
        }
        if resource_id.is_none() {
            let refs = crate::state::instances::adapters::sqlite::content_rows::get_content_provider_refs(&entry_id, pool).await?;
            let origin = crate::state::instances::adapters::sqlite::content_rows::get_content_origin_provider(&entry_id, pool).await?;
            if let Some(reference) = refs
                .iter()
                .find(|reference| Some(reference.provider()) == origin)
                .or_else(|| refs.first())
            {
                let project = SourceProjectRef {
                    source: SourceId::new(reference.provider().as_str())?,
                    project_id: SourceProjectId::new(
                        reference.database_project_id(),
                    )?,
                };
                if crate::state::instances::adapters::sqlite::resource_rows::validate_project(&project).is_ok() {
                    resource_id = Some(crate::state::resources::ResourceStore::new(pool.clone()).ensure_project(&project, &[resource_type]).await?);
                }
            }
        }
        let artifact = exact
            .as_ref()
            .map(|file| file.reference.clone())
            .or_else(|| {
                let link = links.first()?;
                let artifact_id =
                    link.get::<Option<String>, _>("artifact_id")?;
                Some(SourceArtifactRef {
                    release: SourceReleaseRef {
                        project: SourceProjectRef {
                            source: SourceId::new(
                                link.get::<String, _>("source"),
                            )
                            .ok()?,
                            project_id: SourceProjectId::new(
                                link.get::<String, _>("project_id"),
                            )
                            .ok()?,
                        },
                        release_id: SourceReleaseId::new(
                            link.get::<Option<String>, _>("release_id")?,
                        )
                        .ok()?,
                    },
                    artifact_id: SourceArtifactId::new(artifact_id).ok()?,
                })
            });
        let ownership: String = row.get("ownership_kind");
        result.push(InstalledResource {
            entry_id: InstalledEntryId::new(entry_id)?,
            instance_id: instance_id.into(),
            resource_id,
            artifact: artifact.clone(),
            known_artifacts: artifact.into_iter().collect(),
            integrity: local_integrity,
            relative_path: path,
            original_file_name: exact.and_then(|file| file.file_name),
            resource_type,
            scope,
            enabled: row.get::<i64, _>("enabled") != 0,
            missing: row.get::<i64, _>("missing") != 0,
            ownership: if ownership == "pack_managed" {
                ResourceOwnership::PackManaged {
                    pack_member_id: row
                        .get::<Option<String>, _>("member_id")
                        .ok_or_else(|| {
                        invalid(
                            "pack-managed entry has no persistent pack member",
                        )
                    })?,
                }
            } else if ownership == "user_added" {
                ResourceOwnership::UserAdded
            } else {
                ResourceOwnership::LocalDiscovered
            },
        });
    }
    Ok(result)
}

impl ResourceService {
    async fn installed_dependency(
        self: &Arc<Self>,
        selection: &ResourceSelection,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
        constraints: &[String],
        ignore_version_constraints: bool,
    ) -> ResourceResult<
        Option<(ResourceRelease, ResourceArtifact, ReleaseSelectionBasis)>,
    > {
        for entry in installed.iter().filter(|entry| {
            !entry.missing
                && entry.enabled
                && entry.scope == selection.scope
                && entry.resource_type == selection.resource_type
        }) {
            let same_resource = match &selection.resource {
                ResourceRef::Resource { id } => {
                    let current = if let Some(store) = self.store() {
                        match &entry.resource_id {
                            Some(id) => store.resolve(id).await?,
                            None => None,
                        }
                    } else {
                        entry.resource_id.clone()
                    };
                    current.as_ref() == Some(id)
                }
                ResourceRef::Source { project } => entry
                    .known_artifacts
                    .iter()
                    .chain(entry.artifact.iter())
                    .any(|artifact| &artifact.release.project == project),
            };
            if !same_resource {
                continue;
            }
            let selected = if ignore_version_constraints {
                let Some(reference) = &entry.artifact else {
                    continue;
                };
                let release =
                    self.release(&reference.release, None, cancel).await?;
                let assessment = assess_compatibility(
                    &release,
                    target,
                    selection.resource_type,
                );
                if assessment.game_version == CompatibilityState::Incompatible
                    || assessment.loader == CompatibilityState::Incompatible
                {
                    continue;
                }
                let Some(artifact) = release
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        &artifact.reference == reference
                            && identity::exact_integrity(
                                &entry.integrity,
                                &artifact.integrity,
                            )
                    })
                    .cloned()
                else {
                    continue;
                };
                // An enabled local dependency is usable even when its release
                // differs from the dependency metadata requested by newly
                // selected content. Explicit compatibility mismatches remain
                // blocking; missing compatibility metadata does not.
                (
                    release,
                    artifact,
                    ReleaseSelectionBasis::InstalledCompatible,
                )
            } else if matches!(selection.release, ReleaseSelection::Automatic) {
                let Some(reference) = &entry.artifact else {
                    continue;
                };
                let release =
                    self.release(&reference.release, None, cancel).await?;
                let assessment = assess_compatibility(
                    &release,
                    target,
                    selection.resource_type,
                );
                if assessment.game_version != CompatibilityState::Compatible
                    || assessment.loader != CompatibilityState::Compatible
                    || !constraints.iter().all(|constraint| {
                        matches_release_constraint(constraint, &release.version)
                    })
                {
                    continue;
                }
                let Some(artifact) = release
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        &artifact.reference == reference
                            && identity::exact_integrity(
                                &entry.integrity,
                                &artifact.integrity,
                            )
                    })
                    .cloned()
                else {
                    continue;
                };
                (
                    release,
                    artifact,
                    ReleaseSelectionBasis::InstalledCompatible,
                )
            } else {
                self.selected_release_with_constraints(
                    selection,
                    target,
                    &self.preferred_source().await?,
                    cancel,
                    None,
                    constraints,
                )
                .await?
            };
            if identity::exact_integrity(
                &entry.integrity,
                &selected.1.integrity,
            ) {
                return Ok(Some(selected));
            }
        }
        Ok(None)
    }

    async fn normalize_plan_nodes(
        &self,
        draft: &mut OperationDraft,
    ) -> ResourceResult<()> {
        if let Some(store) = self.store() {
            for node in &mut draft.plan.nodes {
                if let ResourceRef::Resource { id } = &mut node.resource
                    && let Some(current) = store.resolve(id).await?
                {
                    *id = current;
                }
            }
        }
        let mut keys = HashMap::<SelectionKey, SelectionId>::new();
        let mut aliases = HashMap::new();
        let mut nodes = Vec::<ResourceDependencyNode>::new();
        for node in std::mem::take(&mut draft.plan.nodes) {
            let entry = draft
                .plan
                .selections
                .iter()
                .find(|selection| selection.id == node.id)
                .and_then(|selection| selection.entry_id.as_ref());
            let key = SelectionKey {
                resource: node.resource.clone(),
                scope: node.scope.clone(),
                kind: node.resource_type.unwrap_or(ResourceType::Mod),
                entry: entry.cloned(),
            };
            if let Some(previous) = keys.get(&key) {
                let existing = nodes
                    .iter_mut()
                    .find(|existing| &existing.id == previous)
                    .unwrap();
                if !existing
                    .artifact
                    .as_ref()
                    .zip(node.artifact.as_ref())
                    .is_some_and(|(left, right)| {
                        identity::exact_integrity(
                            &left.integrity,
                            &right.integrity,
                        )
                    })
                {
                    existing.state = DependencyNodeState::Conflict;
                    issue(
                        &mut draft.plan,
                        ResourceIssueCode::VersionConflict,
                        previous,
                        true,
                        "confirmed resource requires conflicting artifacts",
                    );
                }
                existing.root_selection_ids.extend(node.root_selection_ids);
                existing.installed_entries.extend(node.installed_entries);
                existing.root_selection_ids.sort();
                existing.root_selection_ids.dedup();
                existing.installed_entries.sort();
                existing.installed_entries.dedup();
                aliases.insert(node.id.clone(), previous.clone());
                draft
                    .plan
                    .actions
                    .retain(|action| action.node_id != node.id);
            } else {
                keys.insert(key, node.id.clone());
                nodes.push(node);
            }
        }
        for edge in &mut draft.plan.edges {
            if let Some(id) = aliases.get(&edge.child) {
                edge.child = id.clone();
            }
            if let Some(id) = aliases.get(&edge.parent) {
                edge.parent = id.clone();
            }
        }
        for issue in &mut draft.plan.issues {
            if let Some(id) =
                issue.selection_id.as_ref().and_then(|id| aliases.get(id))
            {
                issue.selection_id = Some(id.clone());
            }
        }
        draft.plan.nodes = nodes;
        Ok(())
    }

    async fn canonical(
        self: &Arc<Self>,
        selection: &mut ResourceSelection,
        cancel: &CancellationToken,
    ) -> ResourceResult<SourceProjectMetadata> {
        validate_scope(&selection.scope, selection.resource_type)?;
        let resource =
            self.get(&selection.resource, None, None, cancel).await?;
        let metadata = resource
            .projects
            .iter()
            .find(|project| project.project == resource.default_project)
            .unwrap()
            .clone();
        self.canonical_project(selection, metadata, resource.id, cancel)
            .await
    }

    async fn canonical_project(
        self: &Arc<Self>,
        selection: &mut ResourceSelection,
        metadata: SourceProjectMetadata,
        known_id: Option<ResourceId>,
        cancel: &CancellationToken,
    ) -> ResourceResult<SourceProjectMetadata> {
        validate_scope(&selection.scope, selection.resource_type)?;
        self.confirm_counterpart(&metadata, cancel).await?;
        if !metadata.types.contains(&selection.resource_type) {
            return Err(invalid(
                "selected project does not provide this content type",
            ));
        }
        let id = if let Some(store) = self.store() {
            Some(
                store
                    .ensure_project(&metadata.project, &metadata.types)
                    .await?,
            )
        } else {
            known_id
        };
        if let Some(id) = id {
            selection.resource = ResourceRef::Resource { id };
        }
        Ok(metadata)
    }

    pub(crate) async fn selected_release_for_current(
        self: &Arc<Self>,
        selection: &ResourceSelection,
        target: &ResourceTarget,
        preferred: &SourceId,
        cancel: &CancellationToken,
        current: Option<&ResourceRelease>,
    ) -> ResourceResult<(
        ResourceRelease,
        ResourceArtifact,
        ReleaseSelectionBasis,
    )> {
        self.selected_release_with_constraints(
            selection,
            target,
            preferred,
            cancel,
            current,
            &[],
        )
        .await
    }

    async fn selected_release_with_constraints(
        self: &Arc<Self>,
        selection: &ResourceSelection,
        target: &ResourceTarget,
        preferred: &SourceId,
        cancel: &CancellationToken,
        current: Option<&ResourceRelease>,
        constraints: &[String],
    ) -> ResourceResult<(
        ResourceRelease,
        ResourceArtifact,
        ReleaseSelectionBasis,
    )> {
        self.selected_release_with_requirements(
            selection,
            target,
            preferred,
            cancel,
            current,
            constraints,
            &[],
        )
        .await
    }

    pub(crate) async fn selected_release_with_requirements(
        self: &Arc<Self>,
        selection: &ResourceSelection,
        target: &ResourceTarget,
        preferred: &SourceId,
        cancel: &CancellationToken,
        current: Option<&ResourceRelease>,
        constraints: &[String],
        requirements: &[RetainedRequirement],
    ) -> ResourceResult<(
        ResourceRelease,
        ResourceArtifact,
        ReleaseSelectionBasis,
    )> {
        let mut dependency_target;
        let target = if selection.id.as_str().starts_with("dependency:") {
            dependency_target = target.clone();
            dependency_target.channels.clear();
            &dependency_target
        } else {
            target
        };
        let (_, projects) = self.source_projects(&selection.resource).await?;
        let (release, basis) = match &selection.release {
            ReleaseSelection::Release { reference } => {
                if !projects.contains(&reference.project) {
                    return Err(invalid("release belongs to another resource"));
                }
                (
                    self.release(reference, None, cancel).await?,
                    ReleaseSelectionBasis::FixedByUser,
                )
            }
            ReleaseSelection::Artifact { reference } => {
                if !projects.contains(&reference.release.project) {
                    return Err(invalid(
                        "artifact belongs to another resource",
                    ));
                }
                (
                    self.release(&reference.release, None, cancel).await?,
                    ReleaseSelectionBasis::FixedByUser,
                )
            }
            ReleaseSelection::Automatic => {
                let mut releases = vec![];
                let mut failures = Vec::new();
                let mut catalog_target = target.clone();
                if matches!(
                    selection.resource_type,
                    ResourceType::ResourcePack
                        | ResourceType::Shader
                        | ResourceType::DataPack
                        | ResourceType::World
                ) {
                    catalog_target.loaders.clear();
                }
                if current.is_none() {
                    catalog_target.channels.clear();
                }
                for project in projects {
                    let mut cursor = None;
                    let mut seen = HashSet::new();
                    loop {
                        let slug = if let Some(store) = self.store() {
                            store.project_alias(&project).await?
                        } else {
                            None
                        };
                        let response = self
                            .query(
                                &project.source,
                                ProviderQuery::Releases {
                                    project_id: project.project_id.clone(),
                                    slug,
                                    target: catalog_target.clone(),
                                    cursor: cursor.clone(),
                                    page_size: 50,
                                },
                                CachePolicy::UseFresh,
                                cancel,
                            )
                            .await;
                        let value = match response {
                            Ok(value) => value,
                            Err(ResourceError::Canceled) => {
                                return Err(ResourceError::Canceled);
                            }
                            Err(error) => {
                                failures.push(format!(
                                    "{}: {error}",
                                    project.source.as_str()
                                ));
                                break;
                            }
                        };
                        let ProviderValue::Releases(page) = value.value else {
                            return Err(invalid(
                                "source returned no release page",
                            ));
                        };
                        releases.extend(page.entries.into_iter().filter(
                            |release| {
                                let compatibility = assess_compatibility(
                                    release,
                                    target,
                                    selection.resource_type,
                                );
                                requirements.iter().all(|requirement| {
                                    requirement.accepts(release)
                                }) && compatibility.game_version
                                    == CompatibilityState::Compatible
                                    && compatibility.loader
                                        == CompatibilityState::Compatible
                                    && constraints.iter().all(|constraint| {
                                        matches_release_constraint(
                                            constraint,
                                            &release.version,
                                        )
                                    })
                                    && current.is_none_or(|current| {
                                        !release_is_known_lower(
                                            release, current,
                                        )
                                    })
                            },
                        ));
                        cursor = page.cursor;
                        if page.window_exhausted {
                            return Err(ResourceError::Unsupported("release window exhausted; choose an exact release".into()));
                        }
                        match &cursor {
                            Some(cursor)
                                if seen.len() < 100
                                    && seen.insert(cursor.clone()) => {}
                            None => break,
                            _ => {
                                return Err(invalid(
                                    "release cursor repeated or exceeded its window",
                                ));
                            }
                        }
                    }
                }
                let preferred_channels: Vec<_> = releases
                    .iter()
                    .filter(|release| {
                        target.channels.is_empty()
                            || target.channels.contains(&release.channel)
                    })
                    .cloned()
                    .collect();
                if !preferred_channels.is_empty() {
                    releases = preferred_channels;
                }
                loop {
                    let selected =
                        select_automatic_release(releases.clone(), preferred)
                            .ok_or_else(|| {
                            if failures.is_empty() {
                                return ResourceError::NoCompatibleRelease;
                            }
                            invalid(format!(
                                "no compatible release found for {:?} / {:?}{}",
                                target.game_versions,
                                target.loaders,
                                if failures.is_empty() {
                                    String::new()
                                } else {
                                    format!("; {}", failures.join("; "))
                                }
                            ))
                        })?;
                    if self
                        .local_requirements_accept(
                            &selected.0,
                            requirements,
                            cancel,
                        )
                        .await?
                    {
                        break selected;
                    }
                    releases.retain(|release| {
                        release.reference != selected.0.reference
                    });
                }
            }
        };
        if !constraints.iter().all(|constraint| {
            matches_release_constraint(constraint, &release.version)
        }) {
            return Err(invalid(
                "selected release does not satisfy all dependency version constraints",
            ));
        }
        if !requirements
            .iter()
            .all(|requirement| requirement.accepts(&release))
        {
            return Err(invalid(
                "selected release conflicts with an installed dependent",
            ));
        }
        if !self
            .local_requirements_accept(&release, requirements, cancel)
            .await?
        {
            return Err(invalid(
                "selected mod does not satisfy an installed dependent's version range",
            ));
        }
        let artifact = match &selection.release {
            ReleaseSelection::Artifact { reference } => release
                .artifacts
                .iter()
                .find(|artifact| &artifact.reference == reference),
            _ => {
                let primary: Vec<_> = release
                    .artifacts
                    .iter()
                    .filter(|artifact| artifact.role == ArtifactRole::Primary)
                    .collect();
                if primary.len() == 1 {
                    Some(primary[0])
                } else {
                    None
                }
            }
        }
        .cloned()
        .ok_or_else(|| invalid("select one exact primary artifact"))?;
        if !valid_artifact(&artifact)
            || (artifact.integrity.size.is_none()
                && matches!(
                    artifact.reference.release.project.source.as_str(),
                    "modrinth" | "curseforge"
                ))
            || !artifact
                .integrity
                .hashes
                .iter()
                .any(|hash| hash.algorithm != HashAlgorithm::Md5)
            || artifact.reference.release != release.reference
            || artifact.available == Some(false)
        {
            return Err(invalid("invalid artifact integrity or ownership"));
        }
        Ok((release, artifact, basis))
    }

    async fn dependency_selection(
        self: &Arc<Self>,
        dependency: &ArtifactDependency,
        parent: &ResourceSelection,
        cancel: &CancellationToken,
    ) -> ResourceResult<Option<ResourceSelection>> {
        let (resource, release) = match &dependency.target {
            DependencyTarget::Resource { reference } => {
                (reference.clone(), ReleaseSelection::Automatic)
            }
            DependencyTarget::Release { reference } => (
                ResourceRef::Source {
                    project: reference.project.clone(),
                },
                ReleaseSelection::Release {
                    reference: reference.clone(),
                },
            ),
            DependencyTarget::Artifact { reference } => (
                ResourceRef::Source {
                    project: reference.release.project.clone(),
                },
                ReleaseSelection::Artifact {
                    reference: reference.clone(),
                },
            ),
            DependencyTarget::SourceRelease { source, release_id } => {
                let value = self
                    .query(
                        source,
                        ProviderQuery::Release {
                            project_id: None,
                            release_id: release_id.clone(),
                            slug: None,
                        },
                        CachePolicy::UseFresh,
                        cancel,
                    )
                    .await?;
                let ProviderValue::Release(document) = value.value else {
                    return Err(invalid("dependency release not found"));
                };
                let reference = document.release.reference;
                (
                    ResourceRef::Source {
                        project: reference.project.clone(),
                    },
                    ReleaseSelection::Release { reference },
                )
            }
            _ => return Ok(None),
        };
        // Project types are durable facts, separate from expiring titles and
        // author metadata. Reuse them for dependencies already known locally.
        if let Some(store) = self.store() {
            let projects = match &resource {
                ResourceRef::Source { project } => vec![project.clone()],
                ResourceRef::Resource { id } => store.projects(id).await?,
            };
            let mut types = Vec::new();
            for project in projects {
                types.extend(store.project_types(&project).await?);
            }
            if !types.is_empty() {
                let (resource_type, scope) = dependency_scope(&types, parent)?;
                let resource = self.normalized_reference(&resource).await?;
                let id = SelectionId::new(format!(
                    "dependency:{}:{}:{:?}",
                    serde_json::to_string(&resource)?,
                    serde_json::to_string(&scope)?,
                    resource_type,
                ))?;
                return Ok(Some(ResourceSelection {
                    id,
                    resource,
                    release,
                    resource_type,
                    scope,
                    entry_id: None,
                }));
            }
        }
        let metadata = self.get(&resource, None, None, cancel).await?;
        let known_id = metadata.id.clone();
        let metadata = metadata
            .projects
            .iter()
            .find(|project| project.project == metadata.default_project)
            .ok_or_else(|| invalid("dependency project metadata is missing"))?
            .clone();
        let (resource_type, scope) = dependency_scope(&metadata.types, parent)?;
        let mut selection = ResourceSelection {
            id: parent.id.clone(),
            resource,
            release,
            resource_type,
            scope,
            entry_id: None,
        };
        self.canonical_project(&mut selection, metadata, known_id, cancel)
            .await?;
        selection.id = SelectionId::new(format!(
            "dependency:{}:{}:{:?}",
            serde_json::to_string(&selection.resource)?,
            serde_json::to_string(&selection.scope)?,
            selection.resource_type,
        ))?;
        Ok(Some(selection))
    }

    #[cfg(test)]
    pub(crate) async fn build_operation(
        self: &Arc<Self>,
        draft: OperationDraft,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
    ) -> ResourceResult<OperationDraft> {
        self.build_operation_with_progress(
            draft, target, installed, cancel, None,
        )
        .await
    }

    pub(crate) async fn build_operation_with_progress(
        self: &Arc<Self>,
        draft: OperationDraft,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
        progress: OperationProgress<'_>,
    ) -> ResourceResult<OperationDraft> {
        self.build_operation_with_memo(
            draft, target, installed, cancel, progress, None,
        )
        .await
    }

    pub(crate) async fn build_operation_with_memo(
        self: &Arc<Self>,
        draft: OperationDraft,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
        progress: OperationProgress<'_>,
        memo: Option<&analysis::AnalysisMemo>,
    ) -> ResourceResult<OperationDraft> {
        let mut draft = draft;
        let mut retained_warning = None;
        draft.retained_requirements = if !draft.resolve_dependencies {
            Vec::new()
        } else {
            match if let Some(memo) = memo {
                memo.retained_requirements(self, &draft, installed, cancel)
                    .await
            } else {
                self.retained_requirements(
                    &draft.plan.selections,
                    installed,
                    &draft.local_metadata,
                    cancel,
                )
                .await
            } {
                Ok(requirements) => requirements,
                Err(error)
                    if draft.plan.intent
                        == ResourceOperationIntent::Install
                        && dependency_source_failed(&error) =>
                {
                    retained_warning = Some(error.to_string());
                    Vec::new()
                }
                Err(error) => return Err(error),
            }
        };
        let original = draft.clone();
        let mut seeded: HashMap<SelectionKey, Vec<String>> = HashMap::new();
        for pass in 0..32 {
            tracing::debug!(
                instance_id = draft.plan.instance_id,
                pass = pass + 1,
                "Resolving resource dependency constraints"
            );
            let mut result = self
                .build_operation_pass(
                    original.clone(),
                    target,
                    installed,
                    cancel,
                    progress,
                    &seeded,
                    memo,
                )
                .await?;
            if let Some(message) = &retained_warning {
                for root in result.plan.active_roots.clone() {
                    dependency_analysis_issue(&mut result.plan, &root, message);
                }
            }
            if !result
                .plan
                .issues
                .iter()
                .any(|issue| issue.code == ResourceIssueCode::VersionConflict)
            {
                return Ok(result);
            }
            let mut next: HashMap<SelectionKey, Vec<String>> = HashMap::new();
            let nodes: HashMap<_, _> = result
                .plan
                .nodes
                .iter()
                .map(|node| (&node.id, node))
                .collect();
            for edge in &result.plan.edges {
                if edge.kind != ResourceDependencyKind::Required
                    && !result.plan.active_roots.contains(&edge.child)
                {
                    continue;
                }
                let Some(constraint) = &edge.version_constraint else {
                    continue;
                };
                let Some(node) = nodes.get(&edge.child) else {
                    continue;
                };
                let Some(kind) = node.resource_type else {
                    continue;
                };
                next.entry(SelectionKey {
                    resource: node.resource.clone(),
                    scope: node.scope.clone(),
                    kind,
                    entry: None,
                })
                .or_default()
                .push(constraint.clone());
            }
            for values in next.values_mut() {
                values.sort();
                values.dedup();
            }
            if next == seeded {
                return Ok(result);
            }
            seeded = next;
        }
        Err(invalid("dependency constraints did not converge"))
    }

    async fn build_operation_pass(
        self: &Arc<Self>,
        mut draft: OperationDraft,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
        progress: OperationProgress<'_>,
        seeded: &HashMap<SelectionKey, Vec<String>>,
        memo: Option<&analysis::AnalysisMemo>,
    ) -> ResourceResult<OperationDraft> {
        let preferred = self.preferred_source().await?;
        draft.plan.nodes.clear();
        draft.plan.edges.clear();
        draft.plan.actions.clear();
        draft.plan.issues.clear();
        draft.plan.selection_reasons.clear();
        draft.candidates.clear();
        let mut queue: VecDeque<_> = draft.plan.selections.clone().into();
        let mut resolved: HashMap<SelectionKey, SelectionId> = HashMap::new();
        let mut incompatible = Vec::new();
        let mut seen = HashSet::new();
        if let Some(memo) = memo {
            memo.prefetch(
                self,
                &queue,
                target,
                &preferred,
                &draft.retained_requirements,
                seeded,
                cancel,
            )
            .await?;
        }
        while let Some(mut selection) = queue.pop_front() {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            if seen.len() >= 8192 {
                return Err(invalid(
                    "dependency graph exceeded 8192 resources",
                ));
            }
            if !seen.insert(selection.id.clone()) {
                if let Some(previous) =
                    draft.plan.nodes.iter().find(|node| node.id == selection.id)
                {
                    if draft.plan.intent == ResourceOperationIntent::Install
                        && previous.state == DependencyNodeState::Satisfied
                        && !previous.installed_entries.is_empty()
                    {
                        continue;
                    }
                    let selected = previous.artifact.clone();
                    let matches = if selection_matches_artifact(
                        &selection.release,
                        selected.as_ref(),
                    ) {
                        true
                    } else {
                        let (_, expected, _) = self
                            .selected_release_for_current(
                                &selection, target, &preferred, cancel, None,
                            )
                            .await?;
                        selected.as_ref().is_some_and(|selected| {
                            identity::exact_integrity(
                                &selected.integrity,
                                &expected.integrity,
                            )
                        })
                    };
                    if !matches {
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::VersionConflict,
                            &selection.id,
                            true,
                            "multiple parents require different dependency releases",
                        );
                    }
                }
                continue;
            }
            let metadata =
                match analysis::canonical(self, &mut selection, cancel, memo)
                    .await
                {
                    Ok(metadata) => metadata,
                    Err(ResourceError::Canceled) => {
                        return Err(ResourceError::Canceled);
                    }
                    Err(error) => {
                        if !draft.plan.active_roots.contains(&selection.id)
                            && dependency_source_failed(&error)
                        {
                            dependency_analysis_issue(
                                &mut draft.plan,
                                &selection.id,
                                error.to_string(),
                            );
                            continue;
                        }
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::SourceUnavailable,
                            &selection.id,
                            true,
                            error.to_string(),
                        );
                        continue;
                    }
                };
            if selection.id.as_str().starts_with("dependency:")
                && matches!(selection.release, ReleaseSelection::Automatic)
                && let Some(fixed) = queue.iter().find(|other| {
                    other.id == selection.id
                        && !matches!(other.release, ReleaseSelection::Automatic)
                })
            {
                selection.release = fixed.release.clone();
            }
            let previous_release = if draft.plan.intent
                == ResourceOperationIntent::Update
            {
                if let Some(artifact) = selection
                    .entry_id
                    .as_ref()
                    .and_then(|id| {
                        installed.iter().find(|entry| &entry.entry_id == id)
                    })
                    .and_then(|entry| entry.artifact.as_ref())
                {
                    Some(self.release(&artifact.release, None, cancel).await?)
                } else {
                    None
                }
            } else {
                None
            };
            let mut constraints: Vec<_> = draft
                .plan
                .edges
                .iter()
                .filter(|edge| {
                    edge.child == selection.id
                        && (edge.kind == ResourceDependencyKind::Required
                            || edge.kind == ResourceDependencyKind::Optional
                                && draft
                                    .plan
                                    .active_roots
                                    .contains(&selection.id))
                })
                .filter_map(|edge| edge.version_constraint.clone())
                .collect();
            if let Some(previous) = seeded.get(&selection_key(&selection)) {
                constraints.extend(previous.iter().cloned());
                constraints.sort();
                constraints.dedup();
            }
            let requirements = self
                .requirements_for(&selection, &draft.retained_requirements)
                .await?;
            let ignore_local_version_constraints = draft.plan.intent
                == ResourceOperationIntent::Install
                && !draft.plan.active_roots.contains(&selection.id)
                && draft.plan.edges.iter().any(|edge| {
                    edge.child == selection.id
                        && edge.kind == ResourceDependencyKind::Required
                });
            let satisfied_dependency =
                if selection.id.as_str().starts_with("dependency:")
                    && selection.entry_id.is_none()
                {
                    match self
                        .installed_dependency(
                            &selection,
                            target,
                            installed,
                            cancel,
                            &constraints,
                            ignore_local_version_constraints,
                        )
                        .await
                    {
                        Ok(value) => value,
                        Err(ResourceError::Canceled) => {
                            return Err(ResourceError::Canceled);
                        }
                        Err(error) => {
                            if draft.plan.active_roots.contains(&selection.id)
                                || !dependency_source_failed(&error)
                            {
                                issue(
                                    &mut draft.plan,
                                    ResourceIssueCode::SourceUnavailable,
                                    &selection.id,
                                    true,
                                    error.to_string(),
                                );
                            } else {
                                dependency_analysis_issue(
                                    &mut draft.plan,
                                    &selection.id,
                                    error.to_string(),
                                );
                            }
                            continue;
                        }
                    }
                } else {
                    None
                };
            let locally_satisfied_dependency = ignore_local_version_constraints
                && satisfied_dependency.is_some();
            let (release, artifact, basis) = match if let Some(selected) =
                satisfied_dependency
            {
                Ok(selected)
            } else {
                analysis::selected_release(
                    self,
                    memo,
                    &selection,
                    target,
                    &preferred,
                    cancel,
                    previous_release.as_ref(),
                    &constraints,
                    &requirements,
                )
                .await
            } {
                Ok(value) => value,
                Err(ResourceError::Canceled) => {
                    return Err(ResourceError::Canceled);
                }
                Err(error) => {
                    if !draft.plan.active_roots.contains(&selection.id)
                        && (constraints.is_empty() && requirements.is_empty()
                            || matches!(
                                error,
                                ResourceError::Network { .. }
                                    | ResourceError::Unavailable
                            ))
                    {
                        dependency_analysis_issue(
                            &mut draft.plan,
                            &selection.id,
                            error.to_string(),
                        );
                        continue;
                    }
                    issue(
                        &mut draft.plan,
                        if matches!(
                            error,
                            ResourceError::Network { .. }
                                | ResourceError::Unavailable
                        ) {
                            ResourceIssueCode::SourceUnavailable
                        } else if constraints.is_empty()
                            && requirements.is_empty()
                        {
                            ResourceIssueCode::MissingDependency
                        } else {
                            ResourceIssueCode::VersionConflict
                        },
                        &selection.id,
                        true,
                        error.to_string(),
                    );
                    continue;
                }
            };
            let local_requirements_match = if locally_satisfied_dependency {
                true
            } else {
                match self
                    .local_requirements_accept(&release, &requirements, cancel)
                    .await
                {
                    Ok(matches) => matches,
                    Err(ResourceError::Canceled) => {
                        return Err(ResourceError::Canceled);
                    }
                    Err(error)
                        if draft.plan.intent
                            == ResourceOperationIntent::Install =>
                    {
                        dependency_analysis_issue(
                            &mut draft.plan,
                            &selection.id,
                            error.to_string(),
                        );
                        true
                    }
                    Err(error) => return Err(error),
                }
            };
            let retained_requirements_match =
                requirements.iter().all(|requirement| {
                    if locally_satisfied_dependency {
                        !requirement.incompatible
                            || requirement.accepts(&release)
                    } else {
                        requirement.accepts(&release)
                    }
                });
            if !retained_requirements_match
                || (!locally_satisfied_dependency && !local_requirements_match)
            {
                issue(
                    &mut draft.plan,
                    ResourceIssueCode::VersionConflict,
                    &selection.id,
                    true,
                    "dependency release conflicts with an installed dependent",
                );
                continue;
            }
            if artifact.reference.release.project.source
                == SourceId::curseforge()
                && self.sources().iter().any(|source| {
                    source.available && source.id == SourceId::modrinth()
                })
            {
                let matches = self
                    .identify_many(
                        &[SourceId::modrinth()],
                        &[identity::LocalArtifactIdentity {
                            integrity: artifact.integrity.clone(),
                            fingerprint: None,
                        }],
                        CachePolicy::UseFresh,
                        cancel,
                    )
                    .await;
                if let Some(matches) =
                    matches.first().filter(|matches| !matches.ambiguous)
                {
                    for other in &matches.matches {
                        if let Some(store) = self.store() {
                            let left = self
                                .project_document(
                                    &artifact.reference.release.project,
                                    None,
                                    cancel,
                                )
                                .await?;
                            let right = self
                                .project_document(
                                    &other.reference.release.project,
                                    None,
                                    cancel,
                                )
                                .await?;
                            store
                                .confirm_projects(
                                    &left.metadata,
                                    &right.metadata,
                                    &artifact,
                                    other,
                                )
                                .await?;
                        }
                    }
                }
                self.canonical(&mut selection, cancel).await?;
            }
            if let Some(store) = self.store() {
                let mut refreshed = HashMap::new();
                for (mut key, id) in std::mem::take(&mut resolved) {
                    if let ResourceRef::Resource { id } = &mut key.resource
                        && let Some(current) = store.resolve(id).await?
                    {
                        *id = current;
                    }
                    refreshed.entry(key).or_insert(id);
                }
                resolved = refreshed;
                for node in &mut draft.plan.nodes {
                    if let ResourceRef::Resource { id } = &mut node.resource
                        && let Some(current) = store.resolve(id).await?
                    {
                        *id = current;
                    }
                }
            }
            if selection.id.as_str().starts_with("dependency:")
                && selection.entry_id.is_none()
                && draft.install_dependencies
                && draft.plan.intent != ResourceOperationIntent::Install
            {
                let mut candidates = Vec::new();
                for entry in installed.iter().filter(|entry| {
                    entry.enabled
                        && !entry.missing
                        && entry.scope == selection.scope
                        && entry.resource_type == selection.resource_type
                }) {
                    let reference = entry
                        .resource_id
                        .as_ref()
                        .map(|id| ResourceRef::Resource { id: id.clone() })
                        .or_else(|| {
                            entry.artifact.as_ref().map(|artifact| {
                                ResourceRef::Source {
                                    project: artifact.release.project.clone(),
                                }
                            })
                        });
                    if let Some(reference) = reference
                        && self.normalized_reference(&reference).await?
                            == selection.resource
                    {
                        candidates.push(entry);
                    }
                }
                if candidates.len() == 1
                    && !identity::exact_integrity(
                        &candidates[0].integrity,
                        &artifact.integrity,
                    )
                {
                    selection.entry_id = Some(candidates[0].entry_id.clone());
                }
            }
            let resource_key = selection_key(&selection);
            let key = SelectionKey {
                entry: selection.entry_id.clone(),
                ..resource_key.clone()
            };
            if let Some(previous) = resolved.get(&key) {
                let node = draft
                    .plan
                    .nodes
                    .iter()
                    .find(|node| &node.id == previous)
                    .unwrap();
                if !node.artifact.as_ref().is_some_and(|other| {
                    identity::exact_integrity(
                        &other.integrity,
                        &artifact.integrity,
                    )
                }) {
                    issue(
                        &mut draft.plan,
                        ResourceIssueCode::VersionConflict,
                        &selection.id,
                        true,
                        "different releases required for the same resource",
                    );
                }
                for edge in &mut draft.plan.edges {
                    if edge.child == selection.id {
                        edge.child = previous.clone();
                    }
                }
                if draft.plan.active_roots.contains(&selection.id) {
                    draft
                        .plan
                        .nodes
                        .iter_mut()
                        .find(|node| &node.id == previous)
                        .unwrap()
                        .root_selection_ids
                        .push(selection.id.clone());
                }
                if let Some(root) = draft
                    .plan
                    .selections
                    .iter_mut()
                    .find(|root| root.id == selection.id)
                {
                    root.resource = selection.resource.clone();
                    root.release = ReleaseSelection::Artifact {
                        reference: artifact.reference.clone(),
                    };
                }
                continue;
            }
            resolved.insert(key, selection.id.clone());
            resolved
                .entry(resource_key)
                .or_insert_with(|| selection.id.clone());
            let assessment =
                assess_compatibility(&release, target, selection.resource_type);
            for (dimension, value) in [
                (CompatibilityDimension::GameVersion, assessment.game_version),
                (CompatibilityDimension::Loader, assessment.loader),
            ] {
                if value == CompatibilityState::Compatible {
                    continue;
                }
                if locally_satisfied_dependency
                    && value == CompatibilityState::Unknown
                {
                    continue;
                }
                let accepted = draft.plan.exceptions.iter().any(|exception| {
                    exception.selection_id == selection.id
                        && exception.release == release.reference
                        && exception.dimensions.contains(&dimension)
                });
                if !accepted {
                    issue(
                        &mut draft.plan,
                        if value == CompatibilityState::Unknown {
                            ResourceIssueCode::CompatibilityUnknown
                        } else {
                            ResourceIssueCode::CompatibilityMismatch
                        },
                        &selection.id,
                        true,
                        format!("{dimension:?}: {value:?}"),
                    );
                }
            }
            let current = selection.entry_id.as_ref().and_then(|entry_id| {
                installed.iter().find(|item| {
                    &item.entry_id == entry_id
                        && item.scope == selection.scope
                        && item.resource_type == selection.resource_type
                })
            });
            if let Some(current) = current {
                let same_resource = match &selection.resource {
                    ResourceRef::Resource { id } => {
                        let current_id =
                            if let (Some(store), Some(current_id)) =
                                (self.store(), current.resource_id.as_ref())
                            {
                                store.resolve(current_id).await?
                            } else {
                                current.resource_id.clone()
                            };
                        current_id.as_ref() == Some(id)
                    }
                    ResourceRef::Source { project } => {
                        current.artifact.as_ref().is_some_and(|artifact| {
                            &artifact.release.project == project
                        })
                    }
                };
                if !same_resource {
                    issue(
                        &mut draft.plan,
                        ResourceIssueCode::ArtifactMismatch,
                        &selection.id,
                        true,
                        "target resource does not own the installed entry",
                    );
                }
                if draft.plan.intent == ResourceOperationIntent::Update {
                    let newer = if let Some(artifact) = &current.artifact {
                        let previous = self
                            .release(&artifact.release, None, cancel)
                            .await?;
                        release_is_newer(&release, &previous)
                    } else {
                        false
                    };
                    if !newer
                        && !identity::exact_integrity(
                            &current.integrity,
                            &artifact.integrity,
                        )
                    {
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::VersionConflict,
                            &selection.id,
                            true,
                            "target is not a newer release; use switch version for an explicit downgrade",
                        );
                    }
                }
            }
            let is_entry_update = draft.plan.intent
                != ResourceOperationIntent::Install
                && selection.entry_id.is_some();
            if is_entry_update && current.is_none() {
                issue(
                    &mut draft.plan,
                    ResourceIssueCode::ArtifactMismatch,
                    &selection.id,
                    true,
                    "the selected installed entry is missing or belongs to another scope",
                );
            }
            let satisfied = installed.iter().find(|item| {
                !item.missing
                    && item.enabled
                    && (!is_entry_update
                        || selection.entry_id.as_ref() == Some(&item.entry_id))
                    && item.scope == selection.scope
                    && item.resource_type == selection.resource_type
                    && identity::exact_integrity(
                        &item.integrity,
                        &artifact.integrity,
                    )
            });
            let needs_confirmation = !draft.install_dependencies
                && !draft.plan.active_roots.contains(&selection.id)
                && satisfied.is_none();
            let mut obstructed_dependency = None;
            if selection.id.as_str().starts_with("dependency:")
                && selection.entry_id.is_none()
                && satisfied.is_none()
            {
                for entry in installed.iter().filter(|entry| {
                    entry.scope == selection.scope
                        && entry.resource_type == selection.resource_type
                }) {
                    let same_resource = match &selection.resource {
                        ResourceRef::Resource { id } => {
                            let entry_id =
                                match (self.store(), &entry.resource_id) {
                                    (Some(store), Some(id)) => {
                                        store.resolve(id).await?
                                    }
                                    _ => entry.resource_id.clone(),
                                };
                            entry_id.as_ref() == Some(id)
                        }
                        ResourceRef::Source { project } => entry
                            .known_artifacts
                            .iter()
                            .chain(entry.artifact.iter())
                            .any(|artifact| {
                                &artifact.release.project == project
                            }),
                    };
                    if same_resource {
                        obstructed_dependency = Some(entry.clone());
                        break;
                    }
                }
            }
            let state = if let Some(entry) = &obstructed_dependency {
                if !entry.enabled && !entry.missing {
                    DependencyNodeState::Disabled
                } else {
                    DependencyNodeState::Conflict
                }
            } else if is_entry_update {
                if current.is_some_and(|current| {
                    !current.enabled
                        && identity::exact_integrity(
                            &current.integrity,
                            &artifact.integrity,
                        )
                        && current.artifact.as_ref()
                            == Some(&artifact.reference)
                }) {
                    DependencyNodeState::Disabled
                } else if satisfied.is_some()
                    && current.is_some_and(|current| {
                        current.artifact.as_ref() == Some(&artifact.reference)
                    })
                {
                    DependencyNodeState::Satisfied
                } else {
                    DependencyNodeState::Update
                }
            } else if satisfied.is_some() {
                DependencyNodeState::Satisfied
            } else if needs_confirmation {
                DependencyNodeState::Missing
            } else if draft.plan.active_roots.contains(&selection.id) {
                DependencyNodeState::SelectedRoot
            } else {
                DependencyNodeState::Install
            };
            let path = artifact_path(
                &selection.scope,
                selection.resource_type,
                &artifact,
            );
            match path {
                Ok(_) if obstructed_dependency.is_some() => {
                    issue(
                        &mut draft.plan,
                        ResourceIssueCode::VersionConflict,
                        &selection.id,
                        true,
                        "installed dependency is disabled, missing or does not satisfy this release; review the installed entry",
                    );
                }
                Ok(_) if needs_confirmation => {
                    issue(
                        &mut draft.plan,
                        ResourceIssueCode::MissingDependency,
                        &selection.id,
                        true,
                        "automatic dependency installation is disabled; explicitly add this required dependency",
                    );
                    draft.candidates.push(ResourceSelection {
                        release: ReleaseSelection::Artifact {
                            reference: artifact.reference.clone(),
                        },
                        ..selection.clone()
                    });
                }
                Ok(_path) if is_entry_update => {
                    if let Some(current) = current {
                        let target_path = super::naming::replacement_path(
                            current,
                            artifact.file_name.as_deref().ok_or_else(|| {
                                invalid("artifact has no filename")
                            })?,
                            draft.naming.preserve,
                        )?;
                        let target_path = if draft.naming.preserve {
                            target_path
                        } else {
                            self.named_path(
                                target_path,
                                &artifact,
                                &draft.naming,
                            )
                            .await?
                        };
                        if installed.iter().any(|item| {
                            item.entry_id != current.entry_id
                                && item
                                    .relative_path
                                    .eq_ignore_ascii_case(&target_path)
                                && !item.missing
                        }) || draft.plan.actions.iter().any(|action| {
                            action
                                .target_relative_path
                                .eq_ignore_ascii_case(&target_path)
                        }) {
                            issue(
                                &mut draft.plan,
                                ResourceIssueCode::InvalidPath,
                                &selection.id,
                                true,
                                format!("target already exists: {target_path}"),
                            );
                            continue;
                        }
                        if current.missing
                            || (draft.plan.intent
                                == ResourceOperationIntent::SwitchVersion
                                && current.artifact.as_ref()
                                    != Some(&artifact.reference))
                            || !identity::exact_integrity(
                                &current.integrity,
                                &artifact.integrity,
                            )
                        {
                            draft.plan.actions.push(ResourceEntryAction {
                                entry_id: current.entry_id.clone(),
                                node_id: selection.id.clone(),
                                kind: ResourceActionKind::Replace,
                                current: Some(current.clone()),
                                target: artifact.clone(),
                                target_relative_path: target_path,
                            });
                        }
                    }
                }
                Ok(path) if satisfied.is_none() => {
                    let path =
                        self.named_path(path, &artifact, &draft.naming).await?;
                    if installed.iter().any(|item| {
                        item.relative_path.eq_ignore_ascii_case(&path)
                            && !item.missing
                    }) || draft.plan.actions.iter().any(|action| {
                        action.target_relative_path.eq_ignore_ascii_case(&path)
                    }) {
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::InvalidPath,
                            &selection.id,
                            true,
                            format!("target already exists: {path}"),
                        );
                    } else {
                        draft.plan.actions.push(ResourceEntryAction {
                            entry_id: InstalledEntryId::new(
                                uuid::Uuid::new_v4().to_string(),
                            )?,
                            node_id: selection.id.clone(),
                            kind: ResourceActionKind::Install,
                            current: None,
                            target: artifact.clone(),
                            target_relative_path: path,
                        });
                    }
                }
                Ok(_) => {}
                Err(error) => issue(
                    &mut draft.plan,
                    ResourceIssueCode::InvalidPath,
                    &selection.id,
                    true,
                    error.to_string(),
                ),
            }
            if !matches!(
                &artifact.acquisition,
                ArtifactAcquisition::PublicDownload { .. }
            ) && draft
                .plan
                .actions
                .iter()
                .any(|action| action.node_id == selection.id)
            {
                issue(
                    &mut draft.plan,
                    ResourceIssueCode::ManualDownloadRequired,
                    &selection.id,
                    true,
                    "source requires an exact manual download",
                );
            }
            draft.plan.nodes.push(ResourceDependencyNode {
                id: selection.id.clone(),
                resource_type: Some(selection.resource_type),
                root_selection_ids: draft
                    .plan
                    .active_roots
                    .iter()
                    .filter(|id| **id == selection.id)
                    .cloned()
                    .collect(),
                resource: selection.resource.clone(),
                scope: selection.scope.clone(),
                state,
                artifact: Some(artifact.clone()),
                installed_entries: satisfied
                    .into_iter()
                    .map(|item| item.entry_id.clone())
                    .chain(
                        obstructed_dependency
                            .iter()
                            .map(|item| item.entry_id.clone()),
                    )
                    .collect(),
                compatibility: assessment,
                declared_compatibility: Some(release.compatibility.clone()),
            });
            draft.plan.selection_reasons.push(ReleaseSelectionReason {
                node_id: selection.id.clone(),
                basis,
            });
            report_node(&draft.plan, Some(metadata), progress);
            if let Some(root) = draft
                .plan
                .selections
                .iter_mut()
                .find(|root| root.id == selection.id)
            {
                root.resource = selection.resource.clone();
                root.release = ReleaseSelection::Artifact {
                    reference: artifact.reference.clone(),
                };
            }
            if !draft.resolve_dependencies {
                continue;
            }
            let dependencies =
                futures::stream::iter(artifact.dependencies.clone())
                    .map(|dependency| {
                        let parent = selection.clone();
                        let service = Arc::clone(self);
                        let cancel = cancel.clone();
                        async move {
                            let child = if matches!(
                                dependency.kind,
                                ResourceDependencyKind::Embedded
                            ) {
                                Ok(None)
                            } else {
                                analysis::dependency_selection(
                                    &service,
                                    memo,
                                    &dependency,
                                    &parent,
                                    &cancel,
                                )
                                .await
                            };
                            (dependency, child)
                        }
                    })
                    .buffered(6)
                    .collect::<Vec<_>>()
                    .await;
            for (dependency, child) in dependencies {
                if dependency.kind == ResourceDependencyKind::Embedded {
                    continue;
                }
                if requirements::local_file_dependency(
                    &mut draft,
                    &selection,
                    &artifact.reference,
                    &dependency,
                    installed,
                )? {
                    continue;
                }
                let child = match child {
                    Ok(Some(child)) => child,
                    Ok(None) => {
                        if dependency.kind == ResourceDependencyKind::Required {
                            dependency_analysis_issue(
                                &mut draft.plan,
                                &selection.id,
                                "dependency requires local metadata verification",
                            );
                        }
                        continue;
                    }
                    Err(ResourceError::Canceled) => {
                        return Err(ResourceError::Canceled);
                    }
                    Err(error) => {
                        if dependency_source_failed(&error)
                            && (dependency.kind
                                == ResourceDependencyKind::Required
                                || draft.plan.intent
                                    == ResourceOperationIntent::Install)
                        {
                            dependency_analysis_issue(
                                &mut draft.plan,
                                &selection.id,
                                error.to_string(),
                            );
                        } else {
                            issue(
                                &mut draft.plan,
                                ResourceIssueCode::SourceUnavailable,
                                &selection.id,
                                dependency.kind
                                    == ResourceDependencyKind::Required,
                                error.to_string(),
                            );
                        }
                        continue;
                    }
                };
                let key = selection_key(&child);
                let id = resolved
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| child.id.clone());
                if let Some(existing) = resolved.get(&key).and_then(|id| {
                    draft.plan.nodes.iter().find(|node| &node.id == id)
                }) && !(draft.plan.intent
                    == ResourceOperationIntent::Install
                    && existing.state == DependencyNodeState::Satisfied
                    && !existing.installed_entries.is_empty())
                    && dependency.kind == ResourceDependencyKind::Required
                    && !selection_matches_artifact(
                        &child.release,
                        existing.artifact.as_ref(),
                    )
                {
                    let existing = existing.artifact.clone();
                    let (_, expected, _) = self
                        .selected_release_for_current(
                            &child, target, &preferred, cancel, None,
                        )
                        .await?;
                    if !existing.as_ref().is_some_and(|existing| {
                        identity::exact_integrity(
                            &existing.integrity,
                            &expected.integrity,
                        )
                    }) {
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::VersionConflict,
                            &selection.id,
                            true,
                            "required dependency conflicts with an active root release or artifact",
                        );
                    }
                }
                draft.plan.edges.push(ResourceDependencyEdge {
                    parent: selection.id.clone(),
                    child: id.clone(),
                    declared_by: artifact.reference.clone(),
                    kind: dependency.kind,
                    release_selection: Some(child.release.clone()),
                    version_constraint: dependency.version_constraint.clone(),
                });
                if dependency.kind == ResourceDependencyKind::Required {
                    if !resolved.contains_key(&key) {
                        queue.push_back(child);
                    }
                } else if dependency.kind == ResourceDependencyKind::Optional {
                    if !draft
                        .candidates
                        .iter()
                        .any(|candidate| candidate.id == id)
                    {
                        draft
                            .candidates
                            .push(ResourceSelection { id, ..child });
                    }
                } else if dependency.kind
                    == ResourceDependencyKind::Incompatible
                {
                    incompatible.push((selection.id.clone(), child));
                }
            }
        }
        self.normalize_plan_nodes(&mut draft).await?;
        let mut candidates = Vec::<ResourceSelection>::new();
        let mut candidate_ids = HashMap::<SelectionKey, SelectionId>::new();
        let mut installed = installed.to_vec();
        if let Some(store) = self.store() {
            for node in &mut draft.plan.nodes {
                if let ResourceRef::Resource { id } = &mut node.resource
                    && let Some(current) = store.resolve(id).await?
                {
                    *id = current;
                }
            }
            for item in &mut installed {
                if let Some(id) = &item.resource_id {
                    item.resource_id = store.resolve(id).await?;
                }
            }
        }
        for mut candidate in draft.candidates.clone() {
            if let Some(store) = self.store() {
                let current = match &candidate.resource {
                    ResourceRef::Source { project } => {
                        store.lookup(project).await?
                    }
                    ResourceRef::Resource { id } => store.resolve(id).await?,
                };
                if let Some(id) = current {
                    candidate.resource = ResourceRef::Resource { id };
                }
            }
            let key = selection_key(&candidate);
            let active = draft.plan.nodes.iter().find(|node| {
                node.resource == candidate.resource
                    && node.scope == candidate.scope
                    && node.resource_type == Some(candidate.resource_type)
            });
            let id = active
                .map(|node| node.id.clone())
                .or_else(|| candidate_ids.get(&key).cloned())
                .unwrap_or_else(|| candidate.id.clone());
            for edge in &mut draft.plan.edges {
                if edge.child == candidate.id {
                    edge.child = id.clone();
                }
            }
            if candidate_ids.contains_key(&key) {
                continue;
            }
            candidate_ids.insert(key, id.clone());
            candidate.id = id;
            if let Some(active) = active {
                if active.state == DependencyNodeState::Missing {
                    candidates.push(candidate);
                }
                continue;
            }
            let constraints: Vec<_> = draft
                .plan
                .edges
                .iter()
                .filter(|edge| edge.child == candidate.id)
                .filter_map(|edge| edge.version_constraint.clone())
                .collect();
            let ignore_local_version_constraints = draft.plan.intent
                == ResourceOperationIntent::Install
                && !draft.plan.active_roots.contains(&candidate.id)
                && draft.plan.edges.iter().any(|edge| {
                    edge.child == candidate.id
                        && edge.kind == ResourceDependencyKind::Required
                });
            let satisfied_artifact = match self
                .installed_dependency(
                    &candidate,
                    target,
                    &installed,
                    cancel,
                    &constraints,
                    ignore_local_version_constraints,
                )
                .await
            {
                Ok(satisfied) => satisfied,
                Err(ResourceError::Canceled) => {
                    return Err(ResourceError::Canceled);
                }
                Err(error) => {
                    if draft.plan.intent == ResourceOperationIntent::Install
                        && dependency_source_failed(&error)
                    {
                        dependency_analysis_issue(
                            &mut draft.plan,
                            &candidate.id,
                            error.to_string(),
                        );
                    } else {
                        issue(
                            &mut draft.plan,
                            ResourceIssueCode::SourceUnavailable,
                            &candidate.id,
                            false,
                            error.to_string(),
                        );
                    }
                    None
                }
            };
            let satisfied: Vec<_> = installed
                .iter()
                .filter(|item| {
                    !item.missing
                        && item.enabled
                        && item.scope == candidate.scope
                        && item.resource_type == candidate.resource_type
                        && satisfied_artifact.as_ref().is_some_and(
                            |(_, artifact, _)| {
                                identity::exact_integrity(
                                    &item.integrity,
                                    &artifact.integrity,
                                )
                            },
                        )
                })
                .map(|item| item.entry_id.clone())
                .collect();
            let compatibility = satisfied_artifact.as_ref().map_or_else(
                || CompatibilityAssessment {
                    game_version: CompatibilityState::Unknown,
                    loader: CompatibilityState::Unknown,
                    reasons: vec![],
                },
                |(release, _, _)| {
                    assess_compatibility(
                        release,
                        target,
                        candidate.resource_type,
                    )
                },
            );
            let declared_compatibility = satisfied_artifact
                .as_ref()
                .map(|(release, _, _)| release.compatibility.clone());
            draft.plan.nodes.push(ResourceDependencyNode {
                id: candidate.id.clone(),
                resource_type: Some(candidate.resource_type),
                root_selection_ids: vec![],
                resource: candidate.resource.clone(),
                scope: candidate.scope.clone(),
                state: if satisfied.is_empty() {
                    DependencyNodeState::OptionalCandidate
                } else {
                    DependencyNodeState::Satisfied
                },
                artifact: satisfied_artifact.map(|(_, artifact, _)| artifact),
                installed_entries: satisfied.clone(),
                compatibility,
                declared_compatibility,
            });
            report_node(&draft.plan, None, progress);
            if satisfied.is_empty() {
                candidates.push(candidate);
            }
        }
        draft.candidates = candidates;
        for (parent, mut child) in incompatible {
            if let (Some(store), ResourceRef::Resource { id }) =
                (self.store(), &mut child.resource)
                && let Some(current) = store.resolve(id).await?
            {
                *id = current;
            }
            let active = draft.plan.nodes.iter().find(|node| {
                node.resource == child.resource
                    && node.scope == child.scope
                    && node.artifact.is_some()
                    && !matches!(
                        node.state,
                        DependencyNodeState::Missing
                            | DependencyNodeState::Disabled
                    )
                    && !draft.plan.actions.iter().any(|action| {
                        action.node_id == node.id
                            && action
                                .current
                                .as_ref()
                                .is_some_and(|current| !current.enabled)
                    })
                    && selection_matches_artifact(
                        &child.release,
                        node.artifact.as_ref(),
                    )
            });
            let installed_conflict = installed.iter().any(|item| {
                !item.missing
                    && item.enabled
                    && item.scope == child.scope
                    && item.resource_type == child.resource_type
                    && !draft.plan.actions.iter().any(|action| {
                        action.current.as_ref().is_some_and(|current| {
                            current.entry_id == item.entry_id
                        })
                    })
                    && match &child.resource {
                        ResourceRef::Resource { id } => {
                            item.resource_id.as_ref() == Some(id)
                        }
                        ResourceRef::Source { project } => {
                            item.artifact.as_ref().is_some_and(|artifact| {
                                artifact.release.project == *project
                            })
                        }
                    }
                    && match &child.release {
                        ReleaseSelection::Automatic => true,
                        ReleaseSelection::Release { reference } => {
                            item.artifact.as_ref().is_some_and(|artifact| {
                                artifact.release == *reference
                            })
                        }
                        ReleaseSelection::Artifact { reference } => {
                            item.artifact.as_ref() == Some(reference)
                        }
                    }
            });
            if let Some(active) = active {
                for edge in &mut draft.plan.edges {
                    if edge.parent == parent
                        && edge.child == child.id
                        && edge.kind == ResourceDependencyKind::Incompatible
                    {
                        edge.child = active.id.clone();
                    }
                }
            }
            if active.is_some() || installed_conflict {
                issue(
                    &mut draft.plan,
                    ResourceIssueCode::VersionConflict,
                    &parent,
                    true,
                    "incompatible resource is active",
                );
            }
        }
        let mut constraint_issues = Vec::new();
        for edge in draft.plan.edges.iter().filter(|edge| {
            matches!(
                edge.kind,
                ResourceDependencyKind::Required
                    | ResourceDependencyKind::Optional
            )
        }) {
            let Some(node) = draft.plan.nodes.iter().find(|node| {
                node.id == edge.child
                    && node.state != DependencyNodeState::OptionalCandidate
            }) else {
                continue;
            };
            let Some(artifact) = &node.artifact else {
                continue;
            };
            let locally_satisfied = draft.plan.intent
                == ResourceOperationIntent::Install
                && edge.kind == ResourceDependencyKind::Required
                && node.state == DependencyNodeState::Satisfied
                && !node.installed_entries.is_empty();
            let version_matches = if locally_satisfied {
                true
            } else if let Some(constraint) = &edge.version_constraint {
                let release = self
                    .release(&artifact.reference.release, None, cancel)
                    .await?;
                matches_release_constraint(constraint, &release.version)
            } else {
                true
            };
            let mut selection_matches = locally_satisfied
                || edge.release_selection.as_ref().is_none_or(|selection| {
                    selection_matches_artifact(selection, Some(artifact))
                });
            if !locally_satisfied
                && !selection_matches
                && let Some(selection) = &edge.release_selection
            {
                let release_ref = match selection {
                    ReleaseSelection::Release { reference } => reference,
                    ReleaseSelection::Artifact { reference } => {
                        &reference.release
                    }
                    ReleaseSelection::Automatic => unreachable!(),
                };
                let release = self.release(release_ref, None, cancel).await?;
                selection_matches = release
                    .artifacts
                    .iter()
                    .filter(|other| match selection {
                        ReleaseSelection::Artifact { reference } => {
                            &other.reference == reference
                        }
                        _ => other.role == ArtifactRole::Primary,
                    })
                    .any(|other| {
                        identity::exact_integrity(
                            &other.integrity,
                            &artifact.integrity,
                        )
                    });
            }
            let enabled_requirement = edge.kind
                != ResourceDependencyKind::Required
                || (node.state != DependencyNodeState::Disabled
                    && !draft.plan.actions.iter().any(|action| {
                        action.node_id == node.id
                            && action
                                .current
                                .as_ref()
                                .is_some_and(|entry| !entry.enabled)
                    }));
            if !version_matches || !selection_matches || !enabled_requirement {
                constraint_issues.push((
                    edge.parent.clone(),
                    edge.kind == ResourceDependencyKind::Required
                        || !node.root_selection_ids.is_empty(),
                ));
            }
        }
        for (parent, blocking) in constraint_issues {
            issue(
                &mut draft.plan,
                ResourceIssueCode::VersionConflict,
                &parent,
                blocking,
                "parent dependency requirement is not satisfied by the selected artifact",
            );
        }
        verify_import_files(&mut draft).await?;
        Ok(draft)
    }
}

pub(crate) async fn verify_import_files(
    draft: &mut OperationDraft,
) -> ResourceResult<()> {
    draft.plan.issues.retain(|issue| {
        issue.code != ResourceIssueCode::ManualDownloadRequired
    });
    let manual: Vec<_> = draft
        .plan
        .actions
        .iter()
        .filter(|action| {
            !matches!(
                action.target.acquisition,
                ArtifactAcquisition::PublicDownload { .. }
            )
        })
        .map(|action| action.node_id.clone())
        .collect();
    for id in manual {
        issue(
            &mut draft.plan,
            ResourceIssueCode::ManualDownloadRequired,
            &id,
            true,
            "artifact requires a manual download",
        );
    }
    for action in &mut draft.plan.actions {
        let Some(path) = draft.import_files.get(&action.node_id) else {
            continue;
        };
        let path = std::path::Path::new(path);
        if !path.is_absolute()
            || !super::execution::matches_file(path, &action.target.integrity)
                .await?
        {
            return Err(invalid(
                "selected local file does not match the exact source artifact",
            ));
        }
        action.target.integrity.size = Some(
            tokio::fs::metadata(path)
                .await
                .map_err(|error| invalid(error.to_string()))?
                .len(),
        );
        if let Some(node) = draft
            .plan
            .nodes
            .iter_mut()
            .find(|node| node.id == action.node_id)
        {
            node.artifact = Some(action.target.clone());
        }
        draft.plan.issues.retain(|issue| {
            issue.code != ResourceIssueCode::ManualDownloadRequired
                || issue.selection_id.as_ref() != Some(&action.node_id)
        });
    }
    Ok(())
}

pub async fn prepare_import_files(
    plan: ResourceOperationPlan,
    files: HashMap<SelectionId, String>,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceOperationPlan> {
    if files
        .keys()
        .any(|id| !plan.selections.iter().any(|selection| &selection.id == id))
    {
        return Err(invalid("local imports must belong to selected roots"));
    }
    let state = crate::State::get().await?;
    prepare_import_files_with_pool(&state.pool, plan, files, cancel).await
}

pub(crate) async fn prepare_import_files_with_pool(
    pool: &SqlitePool,
    plan: ResourceOperationPlan,
    files: HashMap<SelectionId, String>,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceOperationPlan> {
    if files
        .keys()
        .any(|id| !plan.selections.iter().any(|selection| &selection.id == id))
    {
        return Err(invalid("local imports must belong to selected roots"));
    }
    let mut draft = load(pool, &plan.id, plan.revision).await?;
    if files.keys().any(|id| {
        !draft
            .plan
            .selections
            .iter()
            .any(|selection| &selection.id == id)
    }) {
        return Err(invalid("local imports must belong to selected roots"));
    }
    draft.import_files = files;
    verify_import_files(&mut draft).await?;
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    draft.plan.revision = draft
        .plan
        .revision
        .checked_add(1)
        .ok_or_else(|| invalid("plan revision exhausted"))?;
    let updated = sqlx::query("UPDATE resource_operation_plans SET payload = ?, revision = ? WHERE id = ? AND revision = ? AND job_id IS NULL AND queued = 0").bind(serde_json::to_string(&draft)?).bind(draft.plan.revision).bind(plan.id.as_str()).bind(plan.revision).execute(pool).await.map_err(db)?;
    if updated.rows_affected() != 1 {
        return Err(invalid("plan was revised or already queued"));
    }
    Ok(draft.plan)
}

pub(crate) fn update_channels(
    channel: crate::state::ReleaseChannel,
) -> Vec<ReleaseChannel> {
    match channel {
        crate::state::ReleaseChannel::Release => vec![ReleaseChannel::Release],
        crate::state::ReleaseChannel::Beta => {
            vec![ReleaseChannel::Release, ReleaseChannel::Beta]
        }
        crate::state::ReleaseChannel::Alpha => vec![
            ReleaseChannel::Release,
            ReleaseChannel::Beta,
            ReleaseChannel::Alpha,
        ],
    }
}

pub async fn prepare(
    instance_id: String,
    intent: ResourceOperationIntent,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceOperationPlan> {
    prepare_with_progress(
        instance_id,
        intent,
        selections,
        install_dependencies,
        cancel,
        None,
        true,
    )
    .await
}

pub async fn prepare_with_progress(
    instance_id: String,
    intent: ResourceOperationIntent,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    progress: OperationProgress<'_>,
    resolve_dependencies: bool,
) -> ResourceResult<ResourceOperationPlan> {
    // A span would repeat this selection list on every nested HTTP/file log.
    // Keep it on one event and bound batch-install diagnostics.
    tracing::debug!(
        %instance_id, ?intent, selection_count = selections.len(),
        selections = ?selections.iter().take(8).collect::<Vec<_>>(),
        "Preparing resource operation"
    );
    prepare_with_installed(
        instance_id,
        intent,
        selections,
        install_dependencies,
        cancel,
        None,
        progress,
        resolve_dependencies,
    )
    .await
}

pub(crate) async fn prepare_with_installed(
    instance_id: String,
    intent: ResourceOperationIntent,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    verified: Option<&[InstalledResource]>,
    progress: OperationProgress<'_>,
    resolve_dependencies: bool,
) -> ResourceResult<ResourceOperationPlan> {
    let (draft, target, installed) = initialize_operation(
        instance_id,
        intent,
        selections,
        install_dependencies,
        cancel,
        verified,
        resolve_dependencies,
    )
    .await?;
    let state = crate::State::get().await?;
    let revision = draft.plan.instance_revision;
    let mut draft = state
        .resources()
        .await?
        .build_operation_with_progress(
            draft, &target, &installed, cancel, progress,
        )
        .await?;
    super::write_access::review(&mut draft, &state).await?;
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if instance_revision(&state.pool, &draft.plan.instance_id).await?
        != revision
    {
        return Err(invalid("instance changed while preparing the plan"));
    }
    sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, ?, ?, ?)").bind(draft.plan.id.as_str()).bind(&draft.plan.instance_id).bind(draft.plan.revision).bind(serde_json::to_string(&draft)?).execute(&state.pool).await.map_err(db)?;
    Ok(draft.plan)
}

async fn initialize_operation(
    instance_id: String,
    intent: ResourceOperationIntent,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    verified: Option<&[InstalledResource]>,
    resolve_dependencies: bool,
) -> ResourceResult<(OperationDraft, ResourceTarget, Vec<InstalledResource>)> {
    match intent {
        ResourceOperationIntent::Install
            if selections
                .iter()
                .any(|selection| selection.entry_id.is_some()) =>
        {
            return Err(invalid(
                "install roots cannot target an existing entry",
            ));
        }
        ResourceOperationIntent::Update
        | ResourceOperationIntent::SwitchVersion
            if selections
                .iter()
                .all(|selection| selection.entry_id.is_none()) =>
        {
            return Err(invalid("updates must identify an installed entry"));
        }
        _ => {}
    }
    if selections.is_empty()
        || selections.len() > 4096
        || selections
            .iter()
            .map(|selection| &selection.id)
            .collect::<HashSet<_>>()
            .len()
            != selections.len()
    {
        return Err(invalid("select between one and 4096 distinct roots"));
    }
    let state = crate::State::get().await?;
    let instance = crate::state::get_instance(&instance_id, &state.pool)
        .await?
        .ok_or_else(|| invalid("unknown instance"))?;
    if instance.instance.install_stage
        != crate::state::InstanceInstallStage::Installed
    {
        return Err(invalid("instance is not ready for resource changes"));
    }
    if verified.is_none() {
        if intent == ResourceOperationIntent::Install {
            super::installed::refresh_install_snapshot(
                &state,
                &instance_id,
                cancel,
            )
            .await?;
        } else {
            super::installed::refresh_snapshot(&state, &instance_id, cancel)
                .await?;
        }
    }
    let revision = instance_revision(&state.pool, &instance_id).await?;
    let roots = selections
        .iter()
        .map(|selection| selection.id.clone())
        .collect();
    let draft = OperationDraft {
        local_metadata: requirements::local_metadata(&state.pool, &instance_id)
            .await?,
        retained_requirements: Vec::new(),
        resolve_dependencies,
        skip_unwritable_files: false,
        import_files: Default::default(),
        naming: {
            let settings = crate::state::Settings::get(&state.pool).await?;
            super::naming::NamingPreferences {
                chinese: settings.resource_chinese_filenames,
                preserve: settings.resource_preserve_filenames,
            }
        },
        install_dependencies,
        candidates: vec![],
        local_files: HashMap::new(),
        restore_history: None,
        plan: ResourceOperationPlan {
            schema_version: ResourceContractVersion,
            id: PlanId::new(uuid::Uuid::new_v4().to_string())?,
            revision: 1,
            instance_id,
            instance_revision: revision,
            intent,
            selections,
            active_roots: roots,
            nodes: vec![],
            edges: vec![],
            actions: vec![],
            issues: vec![],
            exceptions: vec![],
            selection_reasons: vec![],
        },
    };
    let target = ResourceTarget {
        game_versions: vec![instance.applied_content_set.game_version],
        loaders: vec![instance.applied_content_set.loader.as_str().into()],
        channels: update_channels(instance.instance.update_channel),
    };
    let installed = if let Some(verified) = verified {
        verified.to_vec()
    } else {
        super::installed::hydrate_selected(
            &state,
            &draft.plan.instance_id,
            cancel,
            None,
        )
        .await?
        .entries
    };
    Ok((draft, target, installed))
}

pub(crate) async fn load(
    pool: &SqlitePool,
    id: &PlanId,
    revision: u32,
) -> ResourceResult<OperationDraft> {
    let payload: Option<String> = sqlx::query_scalar("SELECT payload FROM resource_operation_plans WHERE id = ? AND revision = ? AND job_id IS NULL AND queued = 0").bind(id.as_str()).bind(revision).fetch_optional(pool).await.map_err(db)?;
    serde_json::from_str(
        &payload
            .ok_or_else(|| invalid("plan was revised or already queued"))?,
    )
    .map_err(Into::into)
}

impl ResourceService {
    #[cfg(test)]
    pub(crate) async fn revise_operation(
        self: &Arc<Self>,
        draft: OperationDraft,
        decisions: Vec<ResourcePlanDecision>,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
    ) -> ResourceResult<OperationDraft> {
        self.revise_operation_with_progress(
            draft, decisions, target, installed, cancel, None,
        )
        .await
    }

    pub(crate) async fn revise_operation_with_progress(
        self: &Arc<Self>,
        mut draft: OperationDraft,
        decisions: Vec<ResourcePlanDecision>,
        target: &ResourceTarget,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
        progress: OperationProgress<'_>,
    ) -> ResourceResult<OperationDraft> {
        let added_dependency = decisions.iter().any(|decision| {
            matches!(
                decision,
                ResourcePlanDecision::AddOptional { .. }
                    | ResourcePlanDecision::AddRequired { .. }
            )
        });
        let previous_issues = draft.plan.issues.clone();
        for decision in decisions {
            match decision {
                ResourcePlanDecision::IgnoreDependencies => {
                    draft.resolve_dependencies = false;
                    draft.install_dependencies = false;
                }
                ResourcePlanDecision::SkipUnwritableFiles => {
                    draft.skip_unwritable_files = true;
                }
                ResourcePlanDecision::AddRequired { ref node_id }
                | ResourcePlanDecision::AddOptional { ref node_id } => {
                    let expected = if matches!(
                        decision,
                        ResourcePlanDecision::AddRequired { .. }
                    ) {
                        DependencyNodeState::Missing
                    } else {
                        DependencyNodeState::OptionalCandidate
                    };
                    if !draft.plan.nodes.iter().any(|node| {
                        &node.id == node_id && node.state == expected
                    }) {
                        return Err(invalid(
                            "dependency is not an available candidate",
                        ));
                    }
                    let candidate = draft
                        .candidates
                        .iter()
                        .find(|selection| &selection.id == node_id)
                        .ok_or_else(|| invalid("unknown dependency candidate"))?
                        .clone();
                    if !draft
                        .plan
                        .selections
                        .iter()
                        .any(|selection| &selection.id == node_id)
                    {
                        draft.plan.selections.push(candidate);
                        draft.plan.active_roots.push(node_id.clone());
                    }
                }
                ResourcePlanDecision::RemoveOptional { node_id }
                | ResourcePlanDecision::RemoveRoot {
                    selection_id: node_id,
                } => {
                    draft
                        .plan
                        .selections
                        .retain(|selection| selection.id != node_id);
                    draft.plan.active_roots.retain(|root| root != &node_id);
                    draft
                        .plan
                        .exceptions
                        .retain(|exception| exception.selection_id != node_id);
                }
                ResourcePlanDecision::AcceptCompatibility { exception } => {
                    if !draft.plan.nodes.iter().any(|node| {
                        node.id == exception.selection_id
                            && node.artifact.as_ref().is_some_and(|artifact| {
                                artifact.reference.release == exception.release
                            })
                    }) {
                        return Err(invalid(
                            "compatibility exception does not match this frozen release",
                        ));
                    }
                    draft.plan.exceptions.push(exception);
                }
                ResourcePlanDecision::SelectRelease { node_id, selection } => {
                    draft
                        .plan
                        .exceptions
                        .retain(|exception| exception.selection_id != node_id);
                    draft
                        .plan
                        .selections
                        .iter_mut()
                        .find(|root| root.id == node_id)
                        .ok_or_else(|| {
                            invalid("only active roots can change release")
                        })?
                        .release = selection;
                }
            }
        }
        draft.plan.revision = draft
            .plan
            .revision
            .checked_add(1)
            .ok_or_else(|| invalid("plan revision overflow"))?;
        let draft = self
            .build_operation_with_progress(
                draft, target, installed, cancel, progress,
            )
            .await?;
        if added_dependency
            && draft.plan.issues.iter().any(|issue| {
                issue.blocking
                    && matches!(
                        issue.code,
                        ResourceIssueCode::SourceUnavailable
                            | ResourceIssueCode::MissingDependency
                    )
                    && !previous_issues.contains(issue)
                    && !draft.plan.nodes.iter().any(|node| {
                        Some(&node.id) == issue.selection_id.as_ref()
                            && node.state == DependencyNodeState::Missing
                    })
            })
        {
            let reasons = draft
                .plan
                .issues
                .iter()
                .filter(|issue| {
                    issue.blocking && !previous_issues.contains(issue)
                })
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            return Err(invalid(format!(
                "unable to resolve the added dependency; your previous selection is retained: {reasons}",
            )));
        }
        Ok(draft)
    }
}

pub async fn revise(
    id: PlanId,
    revision: u32,
    decisions: Vec<ResourcePlanDecision>,
    cancel: &CancellationToken,
) -> ResourceResult<ResourceOperationPlan> {
    revise_with_progress(id, revision, decisions, cancel, None).await
}

pub async fn revise_with_progress(
    id: PlanId,
    revision: u32,
    decisions: Vec<ResourcePlanDecision>,
    cancel: &CancellationToken,
    progress: OperationProgress<'_>,
) -> ResourceResult<ResourceOperationPlan> {
    let state = crate::State::get().await?;
    let draft = load(&state.pool, &id, revision).await?;
    if instance_revision(&state.pool, &draft.plan.instance_id).await?
        != draft.plan.instance_revision
    {
        return Err(invalid("instance changed; prepare a new plan"));
    }
    let instance =
        crate::state::get_instance(&draft.plan.instance_id, &state.pool)
            .await?
            .ok_or_else(|| invalid("unknown instance"))?;
    let target = ResourceTarget {
        game_versions: vec![instance.applied_content_set.game_version],
        loaders: vec![instance.applied_content_set.loader.as_str().into()],
        channels: update_channels(instance.instance.update_channel),
    };
    let installed = installed(&state.pool, &draft.plan.instance_id).await?;
    let mut draft = state
        .resources()
        .await?
        .revise_operation_with_progress(
            draft, decisions, &target, &installed, cancel, progress,
        )
        .await?;
    super::write_access::review(&mut draft, &state).await?;
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled);
    }
    if instance_revision(&state.pool, &draft.plan.instance_id).await?
        != draft.plan.instance_revision
    {
        return Err(invalid("instance changed while revising the plan"));
    }
    let result = sqlx::query("UPDATE resource_operation_plans SET revision = ?, payload = ? WHERE id = ? AND revision = ? AND job_id IS NULL AND queued = 0").bind(draft.plan.revision).bind(serde_json::to_string(&draft)?).bind(id.as_str()).bind(revision).execute(&state.pool).await.map_err(db)?;
    if result.rows_affected() != 1 {
        return Err(invalid("plan changed concurrently"));
    }
    Ok(draft.plan)
}

pub async fn queue(
    id: PlanId,
    revision: u32,
) -> ResourceResult<crate::install::InstallJobSnapshot> {
    crate::install::runner::queue_resource_plan(id, revision)
        .await
        .map_err(|error| ResourceError::InvalidRequest(error.to_string()))
}

pub(crate) async fn execute_frozen_plan(
    instance_id: &str,
    plan: &ResourceOperationPlan,
    reporter: Option<crate::install::InstallProgressReporter>,
    state: &crate::State,
) -> crate::Result<()> {
    super::execution::execute(instance_id, plan, reporter, state).await
}

pub(crate) async fn frozen_request_completed(
    request: &crate::install::InstallRequest,
    pool: &SqlitePool,
) -> crate::Result<bool> {
    let crate::install::InstallRequest::InstallContentBatch { items, .. } =
        request
    else {
        return Ok(false);
    };
    if items.is_empty() {
        return Ok(false);
    }
    for item in items {
        let crate::install::InstallContentBatchItem::FrozenResource { plan } =
            item
        else {
            return Ok(false);
        };
        let completed: Option<i64> = sqlx::query_scalar("SELECT completed FROM resource_operation_plans WHERE id = ? AND revision = ?")
            .bind(plan.id.as_str()).bind(plan.revision).fetch_optional(pool).await?;
        if completed != Some(1) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(crate) async fn recover_frozen_plan(
    plan: &ResourceOperationPlan,
    state: &crate::State,
) -> crate::Result<()> {
    super::execution::recover_plan(plan, state).await
}
