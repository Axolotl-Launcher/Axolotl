use super::*;
use crate::mod_metadata::LocalModMetadata;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct RetainedRequirement {
    pub parent: InstalledResource,
    pub resource: ResourceRef,
    pub scope: InstallScope,
    pub version_constraint: Option<String>,
    pub exact: Vec<ArtifactIntegrity>,
    pub incompatible: bool,
    pub local_mod_id: Option<String>,
}

impl RetainedRequirement {
    pub(super) fn accepts(&self, release: &ResourceRelease) -> bool {
        let matches =
            self.version_constraint.as_ref().is_none_or(|constraint| {
                self.local_mod_id.is_some()
                    || matches_release_constraint(constraint, &release.version)
            }) && (self.exact.is_empty()
                || release.artifacts.iter().any(|artifact| {
                    artifact.role == ArtifactRole::Primary
                        && self.exact.iter().any(|expected| {
                            identity::exact_integrity(
                                expected,
                                &artifact.integrity,
                            )
                        })
                }));
        matches != self.incompatible
    }
}

pub(crate) fn mod_version_matches(pattern: &str, version: &str) -> bool {
    let pattern = pattern.trim();
    if pattern == version
        || matches!(pattern, "*" | "x" | "X") && !version.is_empty()
    {
        return true;
    }
    if pattern.contains("||") {
        return pattern
            .split("||")
            .any(|part| mod_version_matches(part.trim(), version));
    }
    if matches_release_constraint(pattern, version) {
        return true;
    }
    if !pattern.starts_with(['~', '^', '<', '>', '='])
        && !pattern.contains(['*', 'x', 'X'])
    {
        return false;
    }
    let version = semver::Version::parse(version).or_else(|_| {
        let mut parts: Vec<_> = version.split('.').collect();
        if parts.len() > 3
            || parts.iter().any(|part| {
                part.is_empty()
                    || !part.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return semver::Version::parse("");
        }
        parts.resize(3, "0");
        semver::Version::parse(&parts.join("."))
    });
    let Ok(version) = version else { return false };
    let normalized = pattern.split_whitespace().collect::<Vec<_>>().join(", ");
    semver::VersionReq::parse(&normalized)
        .is_ok_and(|range| range.matches(&version))
}

pub(crate) async fn local_metadata(
    pool: &SqlitePool,
    instance_id: &str,
) -> ResourceResult<HashMap<InstalledEntryId, LocalModMetadata>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT entry.id, file.local_mod_data FROM instance_content_entries entry JOIN instance_files file ON file.id = entry.file_id WHERE entry.instance_id = ? AND file.missing = 0 AND file.local_mod_data IS NOT NULL"
    ).bind(instance_id).fetch_all(pool).await.map_err(db)?;
    rows.into_iter()
        .filter_map(|(id, value)| {
            serde_json::from_str(&value).ok().map(|metadata| {
                InstalledEntryId::new(id)
                    .map(|id| (id, metadata))
                    .map_err(Into::into)
            })
        })
        .collect()
}

impl ResourceService {
    pub(super) async fn normalized_reference(
        &self,
        reference: &ResourceRef,
    ) -> ResourceResult<ResourceRef> {
        let id = if let Some(store) = self.store() {
            match reference {
                ResourceRef::Resource { id } => store.resolve(id).await?,
                ResourceRef::Source { project } => {
                    store.lookup(project).await?
                }
            }
        } else {
            None
        };
        Ok(id
            .map(|id| ResourceRef::Resource { id })
            .unwrap_or_else(|| reference.clone()))
    }

    pub(crate) async fn retained_requirements(
        self: &Arc<Self>,
        selections: &[ResourceSelection],
        installed: &[InstalledResource],
        local: &HashMap<InstalledEntryId, LocalModMetadata>,
        cancel: &CancellationToken,
    ) -> ResourceResult<Vec<RetainedRequirement>> {
        let changing: HashSet<_> = selections
            .iter()
            .filter_map(|selection| selection.entry_id.as_ref())
            .collect();
        let parents: Vec<_> = installed
            .iter()
            .filter(|entry| {
                entry.enabled
                    && !entry.missing
                    && !changing.contains(&entry.entry_id)
            })
            .collect();
        let mut saved_artifacts = HashMap::new();
        if let Some(store) = self.store() {
            let instance_ids: HashSet<_> =
                parents.iter().map(|parent| &parent.instance_id).collect();
            for instance_id in instance_ids {
                saved_artifacts
                    .extend(store.installed_artifacts(instance_id).await?);
            }
        }
        saved_artifacts.retain(|id, artifact| {
            installed.iter().any(|parent| {
                !parent.missing
                    && parent.entry_id.as_str() == id.as_str()
                    && parent.artifact.as_ref() == Some(&artifact.reference)
                    && super::super::valid_artifact(artifact)
                    && identity::exact_integrity(
                        &parent.integrity,
                        &artifact.integrity,
                    )
            })
        });
        // A large instance can have hundreds of retained parents. Fetch their
        // exact declarations in provider batches rather than making a round
        // trip for each parent before the first new dependency is shown.
        let references: HashSet<_> = parents
            .iter()
            .filter_map(|parent| {
                if saved_artifacts.contains_key(parent.entry_id.as_str()) {
                    return None;
                }
                parent
                    .artifact
                    .as_ref()
                    .map(|artifact| artifact.release.clone())
            })
            .collect();
        let mut releases = HashMap::new();
        let mut by_source: HashMap<SourceId, Vec<SourceReleaseRef>> =
            HashMap::new();
        for reference in references {
            by_source
                .entry(reference.project.source.clone())
                .or_default()
                .push(reference);
        }
        for (source, references) in by_source {
            let mut queries = Vec::with_capacity(references.len());
            for reference in &references {
                let slug = if let Some(store) = self.store() {
                    store.project_alias(&reference.project).await?
                } else {
                    None
                };
                queries.push(ProviderQuery::Release {
                    project_id: Some(reference.project.project_id.clone()),
                    release_id: reference.release_id.clone(),
                    slug,
                });
            }
            let results = self
                .query_many(&source, queries, CachePolicy::UseFresh, cancel)
                .await;
            for (reference, result) in references.into_iter().zip(results) {
                let ProviderValue::Release(document) = result?.value else {
                    return Err(invalid(
                        "installed parent release is unavailable",
                    ));
                };
                if document.release.reference != reference {
                    return Err(invalid(
                        "release does not belong to installed project",
                    ));
                }
                releases.insert(reference, document.release);
            }
        }
        // Dependency metadata determines the install scope. Resolve repeated
        // references once per analysis, including when the disk cache is off.
        let mut dependency_selections: HashMap<
            String,
            Option<ResourceSelection>,
        > = HashMap::new();
        let mut exact_dependencies: HashMap<String, Vec<ArtifactIntegrity>> =
            HashMap::new();
        let mut requirements = Vec::new();
        for parent in parents {
            if cancel.is_cancelled() {
                return Err(ResourceError::Canceled);
            }
            if let Some(reference) = &parent.artifact {
                let artifact = saved_artifacts
                    .get(parent.entry_id.as_str())
                    .or_else(|| {
                        releases.get(&reference.release).and_then(|release| {
                            release.artifacts.iter().find(|artifact| {
                                artifact.reference == *reference
                                    && identity::exact_integrity(
                                        &parent.integrity,
                                        &artifact.integrity,
                                    )
                            })
                        })
                    })
                    .ok_or_else(|| {
                        invalid(format!(
                            "cannot verify dependency declarations for {}",
                            parent.relative_path
                        ))
                    })?;
                let parent_selection = ResourceSelection {
                    id: SelectionId::new(parent.entry_id.as_str())?,
                    resource: ResourceRef::Source {
                        project: reference.release.project.clone(),
                    },
                    release: ReleaseSelection::Artifact {
                        reference: reference.clone(),
                    },
                    resource_type: parent.resource_type,
                    scope: parent.scope.clone(),
                    entry_id: Some(parent.entry_id.clone()),
                };
                for dependency in
                    artifact.dependencies.iter().filter(|dependency| {
                        matches!(
                            dependency.kind,
                            ResourceDependencyKind::Required
                                | ResourceDependencyKind::Incompatible
                        )
                    })
                {
                    let key = serde_json::to_string(&(
                        &dependency.target,
                        &parent.scope,
                        parent.resource_type,
                    ))?;
                    let selection = if let Some(selection) =
                        dependency_selections.get(&key)
                    {
                        selection.clone()
                    } else {
                        let selection = self
                            .dependency_selection(
                                dependency,
                                &parent_selection,
                                cancel,
                            )
                            .await?;
                        dependency_selections
                            .insert(key.clone(), selection.clone());
                        selection
                    };
                    let Some(selection) = selection else {
                        continue;
                    };
                    let exact = if let Some(exact) =
                        exact_dependencies.get(&key)
                    {
                        exact.clone()
                    } else {
                        let cached_exact: Vec<_> = saved_artifacts
                            .values()
                            .filter(|artifact| match &selection.release {
                                ReleaseSelection::Automatic => false,
                                ReleaseSelection::Release { reference } => {
                                    artifact.role == ArtifactRole::Primary
                                        && &artifact.reference.release
                                            == reference
                                }
                                ReleaseSelection::Artifact { reference } => {
                                    &artifact.reference == reference
                                }
                            })
                            .map(|artifact| artifact.integrity.clone())
                            .collect();
                        let exact: Vec<ArtifactIntegrity> = if !cached_exact
                            .is_empty()
                        {
                            cached_exact
                        } else {
                            match &selection.release {
                                ReleaseSelection::Automatic => Vec::new(),
                                ReleaseSelection::Release { reference } => self
                                    .release(reference, None, cancel)
                                    .await?
                                    .artifacts
                                    .into_iter()
                                    .filter(|artifact| {
                                        artifact.role == ArtifactRole::Primary
                                    })
                                    .map(|artifact| artifact.integrity)
                                    .collect(),
                                ReleaseSelection::Artifact { reference } => {
                                    self.release(
                                        &reference.release,
                                        None,
                                        cancel,
                                    )
                                    .await?
                                    .artifacts
                                    .into_iter()
                                    .filter(|artifact| {
                                        artifact.reference == *reference
                                    })
                                    .map(|artifact| artifact.integrity)
                                    .collect()
                                }
                            }
                        };
                        exact_dependencies.insert(key, exact.clone());
                        exact
                    };
                    if !matches!(selection.release, ReleaseSelection::Automatic)
                        && exact.is_empty()
                    {
                        return Err(invalid(
                            "installed parent requires an unavailable artifact",
                        ));
                    }
                    requirements.push(RetainedRequirement {
                        parent: parent.clone(),
                        resource: selection.resource,
                        scope: selection.scope,
                        version_constraint: dependency
                            .version_constraint
                            .clone(),
                        exact,
                        incompatible: dependency.kind
                            == ResourceDependencyKind::Incompatible,
                        local_mod_id: None,
                    });
                }
            }
            if let Some(metadata) = local.get(&parent.entry_id) {
                for dependency in metadata.dependencies.iter().flatten() {
                    for child in installed.iter().filter(|entry| {
                        entry.enabled
                            && !entry.missing
                            && entry.scope == parent.scope
                    }) {
                        if !local.get(&child.entry_id).is_some_and(|meta| {
                            meta.mod_id == dependency.mod_id
                        }) {
                            continue;
                        }
                        let resource = if let Some(id) = &child.resource_id {
                            ResourceRef::Resource { id: id.clone() }
                        } else if let Some(artifact) = &child.artifact {
                            ResourceRef::Source {
                                project: artifact.release.project.clone(),
                            }
                        } else {
                            continue;
                        };
                        requirements.push(RetainedRequirement {
                            parent: parent.clone(),
                            resource: self
                                .normalized_reference(&resource)
                                .await?,
                            scope: child.scope.clone(),
                            version_constraint: dependency
                                .version_range
                                .clone(),
                            exact: Vec::new(),
                            incompatible: false,
                            local_mod_id: Some(dependency.mod_id.clone()),
                        });
                    }
                }
            }
        }
        Ok(requirements)
    }

    pub(crate) async fn requirements_for(
        &self,
        selection: &ResourceSelection,
        requirements: &[RetainedRequirement],
    ) -> ResourceResult<Vec<RetainedRequirement>> {
        let resource = self.normalized_reference(&selection.resource).await?;
        let mut selected = Vec::new();
        for requirement in requirements {
            if selection.entry_id.as_ref() != Some(&requirement.parent.entry_id)
                && requirement.scope == selection.scope
                && self.normalized_reference(&requirement.resource).await?
                    == resource
            {
                selected.push(requirement.clone());
            }
        }
        Ok(selected)
    }
}

pub(super) fn local_file_dependency(
    draft: &mut OperationDraft,
    selection: &ResourceSelection,
    declared_by: &SourceArtifactRef,
    dependency: &ArtifactDependency,
    installed: &[InstalledResource],
) -> ResourceResult<bool> {
    let DependencyTarget::FileName { name } = &dependency.target else {
        return Ok(false);
    };
    if !safe_relative_path(name) || name.contains('/') {
        issue(
            &mut draft.plan,
            ResourceIssueCode::InvalidPath,
            &selection.id,
            dependency.kind == ResourceDependencyKind::Required,
            "dependency filename is invalid",
        );
        return Ok(true);
    }
    let matching: Vec<_> = installed
        .iter()
        .filter(|entry| {
            !entry.missing
                && entry.scope == selection.scope
                && entry.resource_type == selection.resource_type
                && entry.relative_path.rsplit('/').next().is_some_and(|file| {
                    file.trim_end_matches(".disabled") == name
                })
        })
        .collect();
    let enabled: Vec<_> = matching
        .iter()
        .filter(|entry| entry.enabled)
        .copied()
        .collect();
    if dependency.kind == ResourceDependencyKind::Incompatible {
        if !enabled.is_empty() {
            issue(
                &mut draft.plan,
                ResourceIssueCode::VersionConflict,
                &selection.id,
                true,
                format!("incompatible local file is active: {name}"),
            );
        }
        return Ok(true);
    }
    if enabled.len() != 1 {
        if dependency.kind == ResourceDependencyKind::Required {
            issue(
                &mut draft.plan,
                ResourceIssueCode::MissingDependency,
                &selection.id,
                true,
                format!(
                    "required local file is missing, disabled or ambiguous: {name}"
                ),
            );
        }
        return Ok(true);
    }
    let entry = enabled[0];
    if draft.plan.intent != ResourceOperationIntent::Install
        && let Some(constraint) = &dependency.version_constraint
        && !draft
            .local_metadata
            .get(&entry.entry_id)
            .and_then(|meta| meta.version.as_deref())
            .is_some_and(|version| mod_version_matches(constraint, version))
    {
        issue(
            &mut draft.plan,
            ResourceIssueCode::VersionConflict,
            &selection.id,
            dependency.kind == ResourceDependencyKind::Required,
            format!("local file does not satisfy the required version: {name}"),
        );
        return Ok(true);
    }
    let id =
        SelectionId::new(format!("local-file:{}", entry.entry_id.as_str()))?;
    if !draft.plan.nodes.iter().any(|node| node.id == id) {
        draft.plan.nodes.push(ResourceDependencyNode {
            id: id.clone(),
            root_selection_ids: vec![],
            resource_type: Some(entry.resource_type),
            resource: ResourceRef::Source {
                project: SourceProjectRef {
                    source: SourceId::new("local")?,
                    project_id: SourceProjectId::new(name.clone())?,
                },
            },
            scope: entry.scope.clone(),
            state: DependencyNodeState::Satisfied,
            artifact: None,
            installed_entries: vec![entry.entry_id.clone()],
            compatibility: CompatibilityAssessment {
                game_version: CompatibilityState::Unknown,
                loader: CompatibilityState::Unknown,
                reasons: vec![],
            },
            declared_compatibility: None,
        });
    }
    draft.plan.edges.push(ResourceDependencyEdge {
        parent: selection.id.clone(),
        child: id,
        declared_by: declared_by.clone(),
        kind: dependency.kind,
        release_selection: None,
        version_constraint: dependency.version_constraint.clone(),
    });
    Ok(true)
}
