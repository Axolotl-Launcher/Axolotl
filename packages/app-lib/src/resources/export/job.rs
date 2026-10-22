use super::{
    plan::{build, invalid},
    refresh::ExportDraft,
};
use crate::resources::{
    providers::{ProjectDocument, ReleaseDocument},
    *,
};
use std::{collections::HashSet, path::PathBuf, sync::Arc};

/// Fresh evidence belongs to the queued task after its editing session closes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResolutionCheckpoint {
    #[serde(default)]
    pub fingerprints: std::collections::HashMap<String, u32>,
    pub scan: ResourceExportScan,
    pub results: Vec<ExportResolvedFile>,
    pub projects: Vec<ProjectDocument>,
    pub releases: Vec<ReleaseDocument>,
    pub preferred_source: SourceId,
    pub selection_revision: u32,
}

pub struct QueuedExport {
    pub plan: ResourceExportPlan,
    pub output: PathBuf,
    pub job: crate::install::InstallJobSnapshot,
}

impl ExportResolutionCheckpoint {
    pub fn capture(draft: &ExportDraft, preferred_source: SourceId) -> Self {
        Self {
            fingerprints: draft.fingerprints.clone(),
            scan: draft.scan.clone(),
            results: draft.results.values().cloned().collect(),
            projects: draft.projects.values().cloned().collect(),
            releases: draft.releases.values().cloned().collect(),
            preferred_source,
            selection_revision: draft.selection_revision,
        }
    }

    pub fn restore(&self) -> ExportDraft {
        let mut draft = ExportDraft::new(self.scan.clone());
        draft.fingerprints = self.fingerprints.clone();
        draft.results = self
            .results
            .iter()
            .map(|file| (file.snapshot.relative_path.clone(), file.clone()))
            .collect();
        draft.projects = self
            .projects
            .iter()
            .map(|doc| (doc.metadata.project.clone(), doc.clone()))
            .collect();
        draft.releases = self
            .releases
            .iter()
            .map(|doc| (doc.release.reference.clone(), doc.clone()))
            .collect();
        draft.selection_revision = self.selection_revision;
        draft
    }
}

pub async fn queue(
    service: &Arc<ResourceService>,
    session: SessionId,
    selected: Vec<String>,
    format: ResourceExportFormat,
    package_info: ExportPackageInfo,
    choices: Vec<ExportEmbeddedFailureChoice>,
    output: PathBuf,
) -> ResourceResult<crate::install::InstallJobSnapshot> {
    let draft = service.export_draft(&session)?;
    let mut draft = draft.lock().await;
    if let Some(queued) = &draft.queued {
        let root =
            crate::api::instance::get_full_path(&queued.plan.instance_id)
                .await?;
        let output =
            super::writer::validate_output(&root, &queued.plan, &output)
                .await?;
        if selected
            != queued
                .plan
                .files
                .iter()
                .map(|action| {
                    super::plan::snapshot(action).relative_path.clone()
                })
                .collect::<Vec<_>>()
            || format != queued.plan.format
            || package_info != queued.plan.package_info
            || choices != queued.plan.embedded_failure_choices
            || output != queued.output
        {
            return Err(invalid(
                "this export is already queued; reopen the window to start a different export",
            ));
        }
        return Ok(queued.job.clone());
    }
    if package_info.name.trim().is_empty()
        || package_info.version.trim().is_empty()
    {
        return Err(invalid("package name and version are required"));
    }
    let mut unique = HashSet::new();
    let mut files = vec![];
    let originals: std::collections::HashMap<_, _> = draft
        .scan
        .files
        .iter()
        .map(|file| (file.relative_path.as_str(), file))
        .collect();
    for path in selected {
        if !unique.insert(path.clone()) {
            return Err(invalid("duplicate export file"));
        }
        let original =
            originals.get(path.as_str()).copied().ok_or_else(|| {
                invalid("selected file is not part of the export snapshot")
            })?;
        files.push(ExportFileAction::Embed {
            snapshot: draft
                .results
                .get(&path)
                .map(|file| &file.snapshot)
                .unwrap_or(original)
                .clone(),
        });
    }
    let plan = ResourceExportPlan {
        schema_version: ResourceContractVersion,
        id: PlanId::new(uuid::Uuid::new_v4().to_string())?,
        revision: 1,
        session_id: session,
        selection_revision: draft.selection_revision,
        instance_id: draft.scan.instance_id.clone(),
        instance_revision: draft.scan.instance_revision,
        format,
        package_info,
        requirements: draft.scan.requirements.clone(),
        files,
        embedded_failure_choices: choices,
    };
    for choice in &plan.embedded_failure_choices {
        if !plan.files.iter().any(|action| super::plan::snapshot(action).relative_path == choice.snapshot.relative_path)
            || !draft.results.get(&choice.snapshot.relative_path).is_some_and(|file| file.snapshot == choice.snapshot && file.resolutions.iter().any(|resolution| matches!(resolution, ExportSourceResolution::Failed { source, .. } if source == &choice.source))) {
            return Err(invalid("embedding choice no longer matches the failed file"));
        }
    }
    super::formats::manifest(&plan)?;
    let root = crate::api::instance::get_full_path(&plan.instance_id).await?;
    let output = super::writer::validate_output(&root, &plan, &output).await?;
    let checkpoint = ExportResolutionCheckpoint::capture(
        &draft,
        service.preferred_source().await?,
    );
    let job = crate::install::runner::resolve_export_resource(
        plan.clone(),
        checkpoint,
        output.clone(),
    )
    .await?;
    draft.queued = Some(QueuedExport {
        plan,
        output,
        job: job.clone(),
    });
    Ok(job)
}

pub async fn resolve(
    service: &Arc<ResourceService>,
    draft: &mut ExportDraft,
    plan: &ResourceExportPlan,
    root: &std::path::Path,
    preferred: &SourceId,
    known: std::collections::HashMap<String, Vec<SourceArtifactRef>>,
    cancel: &tokio_util::sync::CancellationToken,
    progress: &(dyn Fn(ExportRefreshProgress) + Send + Sync),
) -> ResourceResult<ResourceExportPlan> {
    draft.reuse_frozen = true;
    draft.accepted_failures = plan.embedded_failure_choices.clone();
    draft.selected = plan
        .files
        .iter()
        .map(|action| super::plan::snapshot(action).relative_path.clone())
        .collect();
    if let Ok(mut resolved) = build(
        draft,
        draft.selection_revision,
        plan.format,
        plan.package_info.clone(),
        plan.embedded_failure_choices.clone(),
        preferred,
    ) {
        resolved.id = plan.id.clone();
        return Ok(resolved);
    }
    let revision = draft
        .selection_revision
        .checked_add(1)
        .ok_or_else(|| invalid("export revision exhausted"))?;
    super::refresh::refresh_local(
        service,
        draft,
        root,
        plan.files
            .iter()
            .map(|action| super::plan::snapshot(action).relative_path.clone())
            .collect(),
        revision,
        RequestId::new(plan.id.as_str())?,
        known,
        cancel,
        progress,
    )
    .await?;
    let mut resolved = build(
        draft,
        revision,
        plan.format,
        plan.package_info.clone(),
        plan.embedded_failure_choices.clone(),
        preferred,
    )?;
    resolved.id = plan.id.clone();
    Ok(resolved)
}
