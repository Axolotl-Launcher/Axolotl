use super::refresh::ExportDraft;
use crate::resources::{identity::exact_integrity, providers::public_url, *};
use std::{collections::HashSet, sync::Arc};

pub(crate) fn invalid(message: impl Into<String>) -> ResourceError {
    ResourceError::InvalidRequest(message.into())
}

pub struct PreparedExport {
    pub plan: ResourceExportPlan,
    pub queued: Option<crate::install::InstallJobSnapshot>,
    pub output: Option<std::path::PathBuf>,
}

impl PreparedExport {
    pub(crate) fn queued_for(
        &self,
        output: &std::path::Path,
    ) -> ResourceResult<Option<crate::install::InstallJobSnapshot>> {
        if self.queued.is_some() && self.output.as_deref() != Some(output) {
            return Err(invalid(
                "export plan was queued for another output path",
            ));
        }
        Ok(self.queued.clone())
    }
}

pub fn validate_choices(
    draft: &ExportDraft,
    revision: u32,
    choices: &[ExportEmbeddedFailureChoice],
) -> ResourceResult<()> {
    if revision != draft.selection_revision {
        return Err(invalid(
            "export selection changed; refresh before exporting",
        ));
    }
    for path in &draft.selected {
        let file = draft
            .results
            .get(path)
            .ok_or_else(|| invalid("export analysis is incomplete"))?;
        if file.local_error.is_some() {
            return Err(invalid(
                "an unreadable local file cannot be embedded; retry or remove it",
            ));
        }
        for algorithm in [
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
        ] {
            if !file
                .snapshot
                .integrity
                .hashes
                .iter()
                .any(|hash| hash.algorithm == algorithm)
            {
                return Err(invalid("export file has no frozen integrity"));
            }
        }
        if file.snapshot.identifiable
            && [SourceId::modrinth(), SourceId::curseforge()].iter().any(
                |source| {
                    !file.resolutions.iter().any(|r| match r {
                        ExportSourceResolution::Matched {
                            source: s, ..
                        }
                        | ExportSourceResolution::NoMatch { source: s }
                        | ExportSourceResolution::Failed {
                            source: s, ..
                        } => s == source,
                    })
                },
            )
        {
            return Err(invalid("export analysis is incomplete"));
        }
        for resolution in &file.resolutions {
            if let ExportSourceResolution::Failed { source, .. } = resolution
                && !choices.iter().any(|choice| {
                    choice.snapshot == file.snapshot && &choice.source == source
                })
            {
                return Err(invalid(
                    "failed source lookup requires retry or an explicit embedding choice",
                ));
            }
        }
    }
    if choices.iter().any(|choice| !draft.selected.contains(&choice.snapshot.relative_path) || !draft.results.get(&choice.snapshot.relative_path).is_some_and(|file| file.snapshot == choice.snapshot && file.resolutions.iter().any(|r| matches!(r, ExportSourceResolution::Failed { source, .. } if source == &choice.source)))) {
        return Err(invalid("embedding choice no longer matches the failed file"));
    }
    Ok(())
}

fn environment(
    draft: &ExportDraft,
    file: &ExportResolvedFile,
) -> ArtifactEnvironment {
    let protocol = file
        .resolutions
        .iter()
        .filter_map(|r| match r {
            ExportSourceResolution::Matched { source, artifacts }
                if source == &SourceId::modrinth() =>
            {
                artifacts.first()
            }
            _ => None,
        })
        .find_map(|a| {
            draft
                .projects
                .get(&a.reference.release.project)?
                .protocol
                .as_ref()
        });
    let side = |key| match protocol
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
    {
        Some("required") => ArtifactEnvironmentRequirement::Required,
        Some("optional") => ArtifactEnvironmentRequirement::Optional,
        Some("unsupported") => ArtifactEnvironmentRequirement::Unsupported,
        _ => ArtifactEnvironmentRequirement::Unknown,
    };
    ArtifactEnvironment {
        client: side("client_side"),
        server: side("server_side"),
    }
}

fn cf_path(draft: &ExportDraft, artifact: &ResourceArtifact) -> Option<String> {
    let project = draft.projects.get(&artifact.reference.release.project)?;
    let folder = match project.metadata.types.as_slice() {
        [ResourceType::Mod] => "mods",
        [ResourceType::ResourcePack] => "resourcepacks",
        [ResourceType::Shader] => "shaderpacks",
        _ => return None,
    };
    let filename = artifact.file_name.as_ref()?;
    if filename.contains(['/', '\\']) {
        return None;
    }
    Some(format!("{folder}/{filename}"))
}

pub fn build(
    draft: &ExportDraft,
    revision: u32,
    format: ResourceExportFormat,
    info: ExportPackageInfo,
    choices: Vec<ExportEmbeddedFailureChoice>,
    preferred: &SourceId,
) -> ResourceResult<ResourceExportPlan> {
    validate_choices(draft, revision, &choices)?;
    if info.name.trim().is_empty() || info.version.trim().is_empty() {
        return Err(invalid("package name and version are required"));
    }
    let mut files = vec![];
    let mut cf_seen = HashSet::new();
    for path in &draft.selected {
        let file = &draft.results[path];
        let embed = || ExportFileAction::Embed {
            snapshot: file.snapshot.clone(),
        };
        if path.to_ascii_lowercase().ends_with(".disabled")
            || path.to_ascii_lowercase().ends_with(".old")
            || choices.iter().any(|c| c.snapshot == file.snapshot)
        {
            files.push(embed());
            continue;
        }
        let mut artifacts: Vec<_> = file
            .resolutions
            .iter()
            .filter_map(|r| match r {
                ExportSourceResolution::Matched { artifacts, .. } => {
                    Some(artifacts)
                }
                _ => None,
            })
            .flatten()
            .filter(|a| {
                a.available != Some(false)
                    && exact_integrity(&file.snapshot.integrity, &a.integrity)
            })
            .cloned()
            .collect();
        artifacts.sort_by_key(|a| {
            let source = &a.reference.release.project.source;
            (
                if source == preferred {
                    0
                } else if source == &SourceId::modrinth() {
                    1
                } else if source == &SourceId::curseforge() {
                    2
                } else {
                    3
                },
                format!("{:?}", a.reference),
            )
        });
        let artifact = artifacts.into_iter().find_map(|mut a| {
            let source = &a.reference.release.project.source;
            if format == ResourceExportFormat::CurseForge {
                if source != &SourceId::curseforge()
                    || cf_path(draft, &a).as_deref() != Some(path.as_str())
                {
                    return None;
                }
                if crate::resources::providers::positive_id(
                    a.reference.release.project.project_id.as_str(),
                )
                .is_err()
                    || crate::resources::providers::positive_id(
                        a.reference.release.release_id.as_str(),
                    )
                    .is_err()
                {
                    return None;
                }
                return Some(a);
            }
            if format == ResourceExportFormat::Modrinth
                && source != &SourceId::modrinth()
            {
                return None;
            }
            if let ArtifactAcquisition::PublicDownload { urls } =
                &mut a.acquisition
            {
                urls.retain(|url| public_url(url).is_some());
                if !urls.is_empty() {
                    return Some(a);
                }
            }
            None
        });
        files.push(match artifact {
            Some(artifact) if format == ResourceExportFormat::CurseForge => {
                if cf_seen.insert(artifact.reference.release.clone()) {
                    ExportFileAction::CurseForgeReference {
                        snapshot: file.snapshot.clone(),
                        artifact,
                    }
                } else {
                    embed()
                }
            }
            Some(artifact) => ExportFileAction::Download {
                snapshot: file.snapshot.clone(),
                artifact,
                environment: environment(draft, file),
            },
            None => embed(),
        });
    }
    let plan = ResourceExportPlan {
        schema_version: ResourceContractVersion,
        id: PlanId::new(uuid::Uuid::new_v4().to_string())?,
        revision: 1,
        session_id: draft.scan.session_id.clone(),
        selection_revision: revision,
        instance_id: draft.scan.instance_id.clone(),
        instance_revision: draft.scan.instance_revision,
        format,
        package_info: info,
        requirements: draft.scan.requirements.clone(),
        files,
        embedded_failure_choices: choices,
    };
    super::formats::manifest(&plan)?;
    Ok(plan)
}

pub async fn prepare(
    service: &Arc<ResourceService>,
    session: SessionId,
    revision: u32,
    format: ResourceExportFormat,
    info: ExportPackageInfo,
    choices: Vec<ExportEmbeddedFailureChoice>,
) -> ResourceResult<ResourceExportPlan> {
    let draft = service.export_draft(&session)?;
    let mut draft = draft.lock().await;
    let preferred = service.preferred_source().await?;
    let plan = build(&draft, revision, format, info, choices, &preferred)?;
    draft.touched = std::time::Instant::now();
    service.export_plans.retain(|_, prepared| {
        prepared
            .try_lock()
            .map_or(true, |p| p.plan.session_id != session)
    });
    service.export_plans.insert(
        plan.id.clone(),
        Arc::new(tokio::sync::Mutex::new(PreparedExport {
            plan: plan.clone(),
            queued: None,
            output: None,
        })),
    );
    Ok(plan)
}

pub async fn queue(
    service: &Arc<ResourceService>,
    id: PlanId,
    revision: u32,
    output: std::path::PathBuf,
) -> ResourceResult<crate::install::InstallJobSnapshot> {
    let prepared = service
        .export_plans
        .get(&id)
        .map(|p| p.clone())
        .ok_or_else(|| invalid("export plan expired; prepare it again"))?;
    let mut prepared = prepared.lock().await;
    if prepared.plan.revision != revision {
        return Err(invalid("export plan revision changed"));
    }
    let root =
        crate::api::instance::get_full_path(&prepared.plan.instance_id).await?;
    let output =
        super::writer::validate_output(&root, &prepared.plan, &output).await?;
    if let Some(job) = prepared.queued_for(&output)? {
        return Ok(job);
    }
    let draft = service.export_draft(&prepared.plan.session_id)?;
    let draft = draft.lock().await;
    if prepared.plan.files.iter().any(|action| {
        !draft
            .results
            .get(&snapshot(action).relative_path)
            .is_some_and(|file| &file.snapshot == snapshot(action))
    }) || draft.selection_revision != prepared.plan.selection_revision
        || draft.selected
            != prepared
                .plan
                .files
                .iter()
                .map(|a| snapshot(a).relative_path.clone())
                .collect::<Vec<_>>()
    {
        return Err(invalid("export selection changed; prepare it again"));
    }
    let job = crate::install::runner::export_resource(
        prepared.plan.clone(),
        output.clone(),
    )
    .await?;
    prepared.queued = Some(job.clone());
    prepared.output = Some(output);
    Ok(job)
}

pub fn snapshot(action: &ExportFileAction) -> &ExportFileSnapshot {
    match action {
        ExportFileAction::Embed { snapshot }
        | ExportFileAction::Download { snapshot, .. }
        | ExportFileAction::CurseForgeReference { snapshot, .. } => snapshot,
    }
}
