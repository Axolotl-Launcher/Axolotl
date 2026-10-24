//! Cancellable, in-memory cart analysis. Only reviewed plans are persisted.
use super::*;
use dashmap::DashMap;
use futures::{StreamExt, stream};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

type SelectedRelease =
    (ResourceRelease, ResourceArtifact, ReleaseSelectionBasis);

#[derive(Default)]
pub(crate) struct AnalysisMemo {
    projects: DashMap<String, (ResourceSelection, SourceProjectMetadata)>,
    releases: DashMap<String, SelectedRelease>,
    dependencies: DashMap<String, Option<ResourceSelection>>,
    retained: std::sync::Mutex<Option<Vec<RetainedRequirement>>>,
}

pub(crate) struct AnalysisSession {
    instance_id: String,
    cancel: CancellationToken,
    touched: std::sync::Mutex<Instant>,
    data: Mutex<Option<AnalysisData>>,
}

struct AnalysisData {
    created: Instant,
    context: String,
    draft: OperationDraft,
    installed: Vec<InstalledResource>,
    target: ResourceTarget,
    memo: AnalysisMemo,
    selection_revision: u32,
    request: String,
}

pub(super) async fn canonical(
    service: &Arc<ResourceService>,
    selection: &mut ResourceSelection,
    cancel: &CancellationToken,
    memo: Option<&AnalysisMemo>,
) -> ResourceResult<SourceProjectMetadata> {
    let key = serde_json::to_string(selection)?;
    if let Some(value) = memo.and_then(|memo| memo.projects.get(&key)) {
        *selection = value.0.clone();
        return Ok(value.1.clone());
    }
    let metadata = service.canonical(selection, cancel).await?;
    if let Some(memo) = memo {
        memo.projects
            .insert(key, (selection.clone(), metadata.clone()));
    }
    Ok(metadata)
}

pub(super) async fn selected_release(
    service: &Arc<ResourceService>,
    memo: Option<&AnalysisMemo>,
    selection: &ResourceSelection,
    target: &ResourceTarget,
    preferred: &SourceId,
    cancel: &CancellationToken,
    current: Option<&ResourceRelease>,
    constraints: &[String],
    requirements: &[RetainedRequirement],
) -> ResourceResult<SelectedRelease> {
    let mut constraints = constraints.to_vec();
    constraints.sort();
    constraints.dedup();
    let key = serde_json::to_string(&(
        selection,
        target,
        preferred,
        current,
        &constraints,
        requirements,
    ))?;
    if let Some(value) = memo.and_then(|memo| memo.releases.get(&key)) {
        return Ok(value.clone());
    }
    let value = service
        .selected_release_with_requirements(
            selection,
            target,
            preferred,
            cancel,
            current,
            &constraints,
            requirements,
        )
        .await?;
    if let Some(memo) = memo {
        memo.releases.insert(key, value.clone());
    }
    Ok(value)
}

pub(super) async fn dependency_selection(
    service: &Arc<ResourceService>,
    memo: Option<&AnalysisMemo>,
    dependency: &ArtifactDependency,
    parent: &ResourceSelection,
    cancel: &CancellationToken,
) -> ResourceResult<Option<ResourceSelection>> {
    let key = serde_json::to_string(&(
        dependency,
        &parent.scope,
        parent.resource_type,
    ))?;
    if let Some(value) = memo.and_then(|memo| memo.dependencies.get(&key)) {
        return Ok(value.clone());
    }
    let value = service
        .dependency_selection(dependency, parent, cancel)
        .await?;
    if let Some(memo) = memo {
        memo.dependencies.insert(key, value.clone());
    }
    Ok(value)
}

impl AnalysisMemo {
    pub(super) async fn retained_requirements(
        &self,
        service: &Arc<ResourceService>,
        draft: &OperationDraft,
        installed: &[InstalledResource],
        cancel: &CancellationToken,
    ) -> ResourceResult<Vec<RetainedRequirement>> {
        if let Some(value) = self.retained.lock().unwrap().clone() {
            return Ok(value);
        }
        let value = service
            .retained_requirements(
                &draft.plan.selections,
                installed,
                &draft.local_metadata,
                cancel,
            )
            .await?;
        *self.retained.lock().unwrap() = Some(value.clone());
        Ok(value)
    }

    fn bound(&self) {
        if self.projects.len() + self.releases.len() + self.dependencies.len()
            > 16384
        {
            self.projects.clear();
            self.releases.clear();
            self.dependencies.clear();
        }
    }

    pub(super) async fn prefetch(
        &self,
        service: &Arc<ResourceService>,
        queue: &VecDeque<ResourceSelection>,
        target: &ResourceTarget,
        preferred: &SourceId,
        requirements: &[RetainedRequirement],
        seeded: &HashMap<SelectionKey, Vec<String>>,
        cancel: &CancellationToken,
    ) -> ResourceResult<()> {
        // Fetch independent root metadata concurrently; graph mutations and
        // shared constraints are still applied in deterministic queue order.
        let selections: Vec<_> = queue
            .iter()
            .filter(|selection| {
                !selection.id.as_str().starts_with("dependency:")
            })
            .cloned()
            .collect();
        stream::iter(selections)
            .map(|mut selection| async move {
                canonical(service, &mut selection, cancel, Some(self)).await?;
                let local =
                    service.requirements_for(&selection, requirements).await?;
                let constraints = seeded
                    .get(&selection_key(&selection))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                selected_release(
                    service,
                    Some(self),
                    &selection,
                    target,
                    preferred,
                    cancel,
                    None,
                    constraints,
                    &local,
                )
                .await?;
                Ok::<_, ResourceError>(())
            })
            .buffer_unordered(6)
            .collect::<Vec<_>>()
            .await;
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        Ok(())
    }
}

pub async fn open(instance_id: String) -> ResourceResult<SessionId> {
    let service = crate::State::get().await?.resources().await?.clone();
    service.analysis_sessions.retain(|_, session| {
        let keep = session.touched.lock().unwrap().elapsed()
            < Duration::from_secs(1800);
        if !keep {
            session.cancel.cancel();
        }
        keep
    });
    if service.analysis_sessions.len() >= 64 {
        return Err(invalid("too many active cart analysis sessions"));
    }
    let id = SessionId::new(uuid::Uuid::new_v4().to_string())?;
    service.analysis_sessions.insert(
        id.clone(),
        Arc::new(AnalysisSession {
            instance_id,
            cancel: CancellationToken::new(),
            touched: std::sync::Mutex::new(Instant::now()),
            data: Mutex::new(None),
        }),
    );
    Ok(id)
}

pub async fn close(id: &SessionId) -> ResourceResult<()> {
    let service = crate::State::get().await?.resources().await?.clone();
    if let Some((_, session)) = service.analysis_sessions.remove(id) {
        session.cancel.cancel();
    }
    Ok(())
}

async fn context(
    state: &crate::State,
    instance_id: &str,
) -> ResourceResult<String> {
    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| invalid("unknown instance"))?;
    let settings = crate::state::Settings::get(&state.pool).await?;
    let service = state.resources().await?;
    Ok(serde_json::to_string(&(
        instance_revision(&state.pool, instance_id).await?,
        instance.applied_content_set.game_version,
        instance.applied_content_set.loader.as_str(),
        instance.instance.update_channel,
        settings.resource_chinese_filenames,
        settings.resource_preserve_filenames,
        state.bypass_curseforge_download_restrictions(),
        service.preferred_source().await?,
        service.cache().generation().await,
    ))?)
}

pub async fn update(
    id: &SessionId,
    selection_revision: u32,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    progress: OperationProgress<'_>,
    persist: bool,
    expected_instance_id: Option<&str>,
) -> ResourceResult<ResourceOperationPlan> {
    let state = crate::State::get().await?;
    let service = state.resources().await?.clone();
    let session = service
        .analysis_sessions
        .get(id)
        .map(|value| value.clone())
        .ok_or_else(|| invalid("analysis session expired"))?;
    if expected_instance_id
        .is_some_and(|expected| expected != session.instance_id)
    {
        return Err(invalid(
            "analysis session belongs to a different instance",
        ));
    }
    *session.touched.lock().unwrap() = Instant::now();
    let mut data = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => return Err(ResourceError::Canceled),
        _ = cancel.cancelled() => return Err(ResourceError::Canceled),
        data = session.data.lock() => data,
    };
    let operation_cancel = cancel.child_token();
    let result = tokio::select! {
        biased;
        _ = session.cancel.cancelled() => Err(ResourceError::Canceled),
        result = update_data(&state, &service, &session.instance_id, &mut data, selection_revision, selections, install_dependencies, &operation_cancel, progress, persist) => result,
    };
    operation_cancel.cancel();
    result
}

async fn update_data(
    state: &crate::State,
    service: &Arc<ResourceService>,
    instance_id: &str,
    data: &mut Option<AnalysisData>,
    selection_revision: u32,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    cancel: &CancellationToken,
    progress: OperationProgress<'_>,
    persist: bool,
) -> ResourceResult<ResourceOperationPlan> {
    if selections.is_empty()
        || selections.len() > 4096
        || selections
            .iter()
            .any(|selection| selection.entry_id.is_some())
        || selections
            .iter()
            .map(|selection| &selection.id)
            .collect::<HashSet<_>>()
            .len()
            != selections.len()
    {
        return Err(invalid(
            "select between one and 4096 distinct install roots",
        ));
    }
    if data
        .as_ref()
        .is_some_and(|data| selection_revision < data.selection_revision)
    {
        return Err(ResourceError::Canceled);
    }
    if persist {
        super::super::installed::refresh_install_snapshot(
            state,
            instance_id,
            cancel,
        )
        .await?;
    }
    let request = serde_json::to_string(&(&selections, install_dependencies))?;
    let key = context(state, instance_id).await?;
    if data.as_ref().is_none_or(|data| {
        data.context != key || data.created.elapsed() > Duration::from_secs(300)
    }) {
        if !persist {
            super::super::installed::refresh_install_snapshot(
                state,
                instance_id,
                cancel,
            )
            .await?;
        }
        let initialized_context = context(state, instance_id).await?;
        let installed = super::super::installed::hydrate_selected(
            state,
            instance_id,
            cancel,
            None,
        )
        .await?
        .entries;
        let (draft, target, installed) = initialize_operation(
            instance_id.into(),
            ResourceOperationIntent::Install,
            selections.clone(),
            install_dependencies,
            cancel,
            Some(&installed),
            true,
        )
        .await?;
        if context(state, instance_id).await? != initialized_context {
            return Err(invalid(
                "instance changed while initializing cart analysis",
            ));
        }
        *data = Some(AnalysisData {
            created: Instant::now(),
            context: initialized_context,
            draft,
            target,
            installed,
            memo: AnalysisMemo::default(),
            selection_revision,
            request: String::new(),
        });
    }
    let data = data.as_mut().unwrap();
    data.memo.bound();
    let mut scoped = Vec::new();
    for selection in selections {
        if !persist
            && selection.resource_type == ResourceType::DataPack
            && selection.scope == InstallScope::Instance
        {
            // The target world is chosen in the preview. Warm its remote
            // metadata now without inventing an installation scope.
            service.get(&selection.resource, None, None, cancel).await?;
            selected_release(
                service,
                Some(&data.memo),
                &selection,
                &data.target,
                &service.preferred_source().await?,
                cancel,
                None,
                &[],
                &[],
            )
            .await?;
        } else {
            scoped.push(selection);
        }
    }
    let unchanged = data.request == request
        && !data.draft.plan.nodes.is_empty()
        && !data.draft.plan.issues.iter().any(|issue| {
            matches!(
                issue.code,
                ResourceIssueCode::SourceUnavailable
                    | ResourceIssueCode::DependencyAnalysisFailed
            )
        });
    if !unchanged {
        let mut draft = data.draft.clone();
        draft.plan.selections = scoped;
        draft.plan.active_roots = draft
            .plan
            .selections
            .iter()
            .map(|selection| selection.id.clone())
            .collect();
        draft.plan.exceptions.clear();
        draft.install_dependencies = install_dependencies;
        draft = service
            .build_operation_with_memo(
                draft,
                &data.target,
                &data.installed,
                cancel,
                progress,
                Some(&data.memo),
            )
            .await?;
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        if context(state, instance_id).await? != data.context {
            return Err(invalid("instance changed while preparing the plan"));
        }
        data.draft = draft;
        data.request = request;
    }
    data.selection_revision = selection_revision;
    if context(state, instance_id).await? != data.context {
        return Err(invalid("instance changed while preparing the plan"));
    }
    let mut draft = data.draft.clone();
    if persist {
        draft.plan.id = PlanId::new(uuid::Uuid::new_v4().to_string())?;
        super::super::write_access::review(&mut draft, state).await?;
        if cancel.is_cancelled() {
            return Err(ResourceError::Canceled);
        }
        if context(state, instance_id).await? != data.context {
            return Err(invalid("instance changed while preparing the plan"));
        }
        sqlx::query("INSERT INTO resource_operation_plans(id, instance_id, revision, payload) VALUES (?, ?, ?, ?)").bind(draft.plan.id.as_str()).bind(instance_id).bind(draft.plan.revision).bind(serde_json::to_string(&draft)?).execute(&state.pool).await.map_err(db)?;
    }
    Ok(draft.plan)
}
