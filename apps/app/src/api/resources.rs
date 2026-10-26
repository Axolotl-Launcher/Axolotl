use dashmap::DashMap;
use std::sync::{Arc, LazyLock};
use theseus::resources::favorites::ResourceFavorite;
use theseus::resources::plans::ResourceOperationProgress;
use theseus::resources::providers::{
    ProjectDocument, ProviderQuery, ProviderValue, ReleaseDocument,
};
use theseus::resources::{ipc::*, *};
use tokio_util::sync::CancellationToken;

use super::Result;

static REQUESTS: LazyLock<DashMap<RequestId, CancellationToken>> =
    LazyLock::new(DashMap::new);
static EARLY_CANCELLATIONS: LazyLock<DashMap<RequestId, std::time::Instant>> =
    LazyLock::new(DashMap::new);

struct RequestGuard(RequestId);
impl Drop for RequestGuard {
    fn drop(&mut self) {
        REQUESTS.remove(&self.0);
    }
}

fn request(id: &RequestId) -> Result<(RequestGuard, CancellationToken)> {
    let token = CancellationToken::new();
    match REQUESTS.entry(id.clone()) {
        dashmap::mapref::entry::Entry::Occupied(_) => Err(
            ResourceError::InvalidRequest("request ID already active".into())
                .into(),
        ),
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            entry.insert(token.clone());
            if EARLY_CANCELLATIONS.remove(id).is_some_and(|(_, time)| {
                time.elapsed() < std::time::Duration::from_secs(30)
            }) {
                token.cancel();
            }
            Ok((RequestGuard(id.clone()), token))
        }
    }
}

async fn service() -> Result<Arc<ResourceService>> {
    Ok(theseus::State::get().await?.resources().await?.clone())
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("axolotl-resources")
        .invoke_handler(tauri::generate_handler![
            resource_sources,
            resource_search,
            resource_get,
            resource_get_description,
            resource_list_releases,
            resource_get_release,
            resource_get_changelog,
            resource_list_changelogs,
            resource_cancel,
            resource_preferred_source,
            resource_set_preferred_source,
            resource_cache_info,
            resource_cache_clear,
            resource_locate,
            resource_project_document,
            resource_project_documents,
            resource_project_protocol,
            resource_release_documents,
            resource_favorites,
            resource_favorite_add,
            resource_favorite_remove,
            resource_prepare_operation,
            resource_analysis_open,
            resource_analysis_update,
            resource_analysis_close,
            resource_prepare_entry_operation,
            resource_list_installed,
            resource_check_updates,
            resource_revise_plan,
            resource_queue_plan,
            resource_list_history,
            resource_scan_backups,
            resource_prepare_history_cleanup,
            resource_execute_history_cleanup,
            resource_export_scan,
            resource_export_refresh,
            resource_export_refresh_retry,
            resource_export_close,
            resource_export_plan,
            resource_export_execute,
            resource_export_queue,
            resource_export_cancel,
            resource_rollback
        ])
        .build()
}

#[tauri::command]
pub async fn resource_export_scan(
    instance_id: String,
    request_id: RequestId,
) -> Result<ResourceExportScan> {
    let (_guard, cancel) = request(&request_id)?;
    let scan =
        theseus::resources::export::scan::scan(instance_id, &cancel).await?;
    if cancel.is_cancelled() {
        return Err(ResourceError::Canceled.into());
    }
    service().await?.register_export(scan.clone())?;
    if cancel.is_cancelled() {
        service().await?.close_export(&scan.session_id);
        return Err(ResourceError::Canceled.into());
    }
    Ok(scan)
}

#[tauri::command]
pub async fn resource_export_refresh(
    instance_id: String,
    session_id: SessionId,
    selected_files: Vec<ExportFileSnapshot>,
    selection_revision: u32,
    request_id: RequestId,
    on_progress: tauri::ipc::Channel<ExportRefreshProgress>,
) -> Result<ExportResolutionSession> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(theseus::resources::export::refresh::refresh(
        &service().await?,
        instance_id,
        session_id,
        selected_files
            .into_iter()
            .map(|file| file.relative_path)
            .collect(),
        selection_revision,
        request_id,
        &cancel,
        &move |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?)
}

#[tauri::command]
pub async fn resource_export_refresh_retry(
    session_id: SessionId,
    failed_refs: Vec<ExportEmbeddedFailureChoice>,
    selection_revision: u32,
    request_id: RequestId,
    on_progress: tauri::ipc::Channel<ExportRefreshProgress>,
) -> Result<ExportResolutionSession> {
    let (_guard, cancel) = request(&request_id)?;
    let service = service().await?;
    let draft = service.export_draft(&session_id)?;
    let draft = tokio::select! { biased; _ = cancel.cancelled() => return Err(ResourceError::Canceled.into()), draft = draft.lock() => draft };
    if failed_refs.is_empty() || failed_refs.iter().any(|choice| !draft.selected.contains(&choice.snapshot.relative_path) || !draft.results.get(&choice.snapshot.relative_path).is_some_and(|file| file.snapshot == choice.snapshot && file.resolutions.iter().any(|r| matches!(r, ExportSourceResolution::Failed { source, .. } if source == &choice.source)))) {
        return Err(ResourceError::InvalidRequest("retry no longer matches the failed file snapshot".into()).into());
    }
    let instance_id = draft.scan.instance_id.clone();
    let selected_files = draft.selected.clone();
    drop(draft);
    Ok(theseus::resources::export::refresh::refresh(
        &service,
        instance_id,
        session_id,
        selected_files,
        selection_revision,
        request_id,
        &cancel,
        &move |progress| {
            let _ = on_progress.send(progress);
        },
    )
    .await?)
}

#[tauri::command]
pub async fn resource_export_close(session_id: SessionId) -> Result<()> {
    service().await?.close_export(&session_id);
    Ok(())
}

#[tauri::command]
pub async fn resource_list_installed(
    instance_id: String,
    request_id: RequestId,
) -> Result<Vec<InstalledResource>> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(theseus::resources::installed::list(instance_id, &cancel).await?)
}

#[tauri::command]
pub async fn resource_export_plan(
    session_id: SessionId,
    selection_revision: u32,
    format: ResourceExportFormat,
    package_info: ExportPackageInfo,
    embedded_failure_choices: Vec<ExportEmbeddedFailureChoice>,
) -> Result<ResourceExportPlan> {
    Ok(theseus::resources::export::plan::prepare(
        &service().await?,
        session_id,
        selection_revision,
        format,
        package_info,
        embedded_failure_choices,
    )
    .await?)
}

#[tauri::command]
pub async fn resource_export_execute(
    plan_id: PlanId,
    revision: u32,
    output_path: String,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(theseus::resources::export::plan::queue(
        &service().await?,
        plan_id,
        revision,
        output_path.into(),
    )
    .await?)
}

#[tauri::command]
pub async fn resource_export_queue(
    session_id: SessionId,
    selected_files: Vec<ExportFileSnapshot>,
    format: ResourceExportFormat,
    package_info: ExportPackageInfo,
    embedded_failure_choices: Vec<ExportEmbeddedFailureChoice>,
    output_path: String,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(theseus::resources::export::job::queue(
        &service().await?,
        session_id,
        selected_files
            .into_iter()
            .map(|file| file.relative_path)
            .collect(),
        format,
        package_info,
        embedded_failure_choices,
        output_path.into(),
    )
    .await?)
}

#[tauri::command]
pub async fn resource_export_cancel(
    request_id_or_job_id: String,
) -> Result<()> {
    if let Ok(id) = uuid::Uuid::parse_str(&request_id_or_job_id) {
        let state = theseus::State::get().await?;
        if let Some(job) = theseus::install::store::get(id, &state).await?
            && matches!(
                job.state.request,
                theseus::install::InstallRequest::ExportResource { .. }
            )
        {
            theseus::install::runner::cancel_job(id).await?;
            return Ok(());
        }
    }
    resource_cancel(
        RequestId::new(request_id_or_job_id).map_err(ResourceError::from)?,
    );
    Ok(())
}

#[tauri::command]
pub async fn resource_check_updates<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    instance_id: String,
    request_id: RequestId,
    entry_ids: Option<Vec<InstalledEntryId>>,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<theseus::resources::updates::ResourceUpdateCheck> {
    let (_guard, cancel) = request(&request_id)?;
    let on_progress = on_progress.map(|id| {
        id.channel_on::<_, theseus::resources::updates::ResourceUpdateProgress>(
            webview,
        )
    });
    Ok(theseus::resources::updates::check(
        instance_id,
        entry_ids.as_deref(),
        move |progress| {
            if let Some(channel) = &on_progress {
                let _ = channel.send(progress);
            }
        },
        &cancel,
    )
    .await?)
}

#[tauri::command]
pub async fn resource_prepare_entry_operation<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    instance_id: String,
    intent: ResourceOperationIntent,
    entry_ids: Vec<InstalledEntryId>,
    release: Option<SourceReleaseRef>,
    install_dependencies: bool,
    request_id: RequestId,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    resolve_dependencies: Option<bool>,
) -> Result<ResourceOperationPlan> {
    let (_guard, cancel) = request(&request_id)?;
    let on_progress = on_progress
        .map(|id| id.channel_on::<_, ResourceOperationProgress>(webview));
    let progress = move |value| {
        if let Some(channel) = &on_progress {
            let _ = channel.send(value);
        }
    };
    Ok(
        theseus::resources::installed::prepare_entries_with_progress(
            instance_id,
            intent,
            entry_ids,
            release,
            install_dependencies,
            &cancel,
            Some(&progress),
            resolve_dependencies.unwrap_or(true),
        )
        .await?,
    )
}

#[tauri::command]
pub async fn resource_prepare_operation<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    instance_id: String,
    intent: ResourceOperationIntent,
    selections: Vec<ResourceSelection>,
    install_dependencies: Option<bool>,
    import_files: Option<std::collections::HashMap<SelectionId, String>>,
    locators: Option<std::collections::HashMap<SelectionId, String>>,
    request_id: RequestId,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
    resolve_dependencies: Option<bool>,
    analysis_session_id: Option<SessionId>,
    selection_revision: Option<u32>,
) -> Result<ResourceOperationPlan> {
    let (_guard, cancel) = request(&request_id)?;
    let on_progress = on_progress
        .map(|id| id.channel_on::<_, ResourceOperationProgress>(webview));
    if let Some(locators) = locators {
        let resources = service().await?;
        for selection in &selections {
            let Some(locator) = locators.get(&selection.id) else {
                continue;
            };
            let ResourceRef::Source { project } = &selection.resource else {
                continue;
            };
            let metadata = resources
                .project_document(project, Some(locator.clone()), &cancel)
                .await?
                .metadata;
            if let Some(store) = resources.store() {
                store.ensure_project(project, &metadata.types).await?;
                store.add_alias(project, locator).await?;
            }
        }
    }
    let progress = move |value| {
        if let Some(channel) = &on_progress {
            let _ = channel.send(value);
        }
    };
    if let Some(id) = analysis_session_id {
        if intent != ResourceOperationIntent::Install
            || import_files.is_some()
            || resolve_dependencies == Some(false)
        {
            return Err(ResourceError::InvalidRequest(
                "invalid cart analysis promotion".into(),
            )
            .into());
        }
        return Ok(theseus::resources::plans::analysis::update(
            &id,
            selection_revision.unwrap_or_default(),
            selections,
            install_dependencies.unwrap_or(true),
            &cancel,
            Some(&progress),
            true,
            Some(&instance_id),
        )
        .await?);
    }
    let plan = theseus::resources::plans::prepare_with_progress(
        instance_id,
        intent,
        selections,
        install_dependencies.unwrap_or(true),
        &cancel,
        Some(&progress),
        resolve_dependencies.unwrap_or(true),
    )
    .await?;
    Ok(
        if let Some(files) = import_files.filter(|files| !files.is_empty()) {
            theseus::resources::plans::prepare_import_files(
                plan, files, &cancel,
            )
            .await?
        } else {
            plan
        },
    )
}

#[tauri::command]
pub async fn resource_analysis_open(instance_id: String) -> Result<SessionId> {
    Ok(theseus::resources::plans::analysis::open(instance_id).await?)
}

#[tauri::command]
pub async fn resource_analysis_close(session_id: SessionId) -> Result<()> {
    Ok(theseus::resources::plans::analysis::close(&session_id).await?)
}

#[tauri::command]
pub async fn resource_analysis_update<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    session_id: SessionId,
    selection_revision: u32,
    selections: Vec<ResourceSelection>,
    install_dependencies: bool,
    request_id: RequestId,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<ResourceOperationPlan> {
    let (_guard, cancel) = request(&request_id)?;
    let channel = on_progress
        .map(|id| id.channel_on::<_, ResourceOperationProgress>(webview));
    let progress = move |value| {
        if let Some(channel) = &channel {
            let _ = channel.send(value);
        }
    };
    Ok(theseus::resources::plans::analysis::update(
        &session_id,
        selection_revision,
        selections,
        install_dependencies,
        &cancel,
        Some(&progress),
        false,
        None,
    )
    .await?)
}

#[tauri::command]
pub async fn resource_revise_plan<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    plan_id: PlanId,
    revision: u32,
    decisions: Vec<ResourcePlanDecision>,
    request_id: RequestId,
    on_progress: Option<tauri::ipc::JavaScriptChannelId>,
) -> Result<ResourceOperationPlan> {
    let (_guard, cancel) = request(&request_id)?;
    let on_progress = on_progress
        .map(|id| id.channel_on::<_, ResourceOperationProgress>(webview));
    let progress = move |value| {
        if let Some(channel) = &on_progress {
            let _ = channel.send(value);
        }
    };
    Ok(theseus::resources::plans::revise_with_progress(
        plan_id,
        revision,
        decisions,
        &cancel,
        Some(&progress),
    )
    .await?)
}

#[tauri::command]
pub async fn resource_queue_plan(
    plan_id: PlanId,
    revision: u32,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(theseus::resources::plans::queue(plan_id, revision).await?)
}

#[tauri::command]
pub async fn resource_list_history(
    instance_id: String,
    entry_id: Option<InstalledEntryId>,
) -> Result<Vec<ResourceHistoryEntry>> {
    Ok(theseus::resources::history::list(instance_id, entry_id).await?)
}

#[tauri::command]
pub async fn resource_rollback(
    instance_id: String,
    entry_id: InstalledEntryId,
    history_id: HistoryId,
) -> Result<theseus::install::InstallJobSnapshot> {
    Ok(
        theseus::resources::history::rollback(
            instance_id,
            entry_id,
            history_id,
        )
        .await?,
    )
}

#[tauri::command]
pub async fn resource_scan_backups(
    instance_id: String,
) -> Result<Vec<ResourceBackupFile>> {
    Ok(theseus::resources::history_cleanup::scan(instance_id).await?)
}

#[tauri::command]
pub async fn resource_prepare_history_cleanup(
    instance_id: String,
    history_ids: Vec<HistoryId>,
) -> Result<ResourceHistoryCleanupPlan> {
    Ok(
        theseus::resources::history_cleanup::prepare(instance_id, history_ids)
            .await?,
    )
}

#[tauri::command]
pub async fn resource_execute_history_cleanup(
    cleanup_plan_id: PlanId,
    revision: u32,
) -> Result<ResourceHistoryCleanupResult> {
    Ok(
        theseus::resources::history_cleanup::execute(cleanup_plan_id, revision)
            .await?,
    )
}

#[tauri::command]
pub async fn resource_favorites() -> Result<Vec<ResourceFavorite>> {
    Ok(theseus::resources::favorites::list().await?)
}

#[tauri::command]
pub async fn resource_favorite_add(
    project: SourceProjectRef,
    content_type: ResourceType,
    locator: Option<String>,
) -> Result<()> {
    theseus::resources::favorites::add(project, content_type, locator).await?;
    Ok(())
}

#[tauri::command]
pub async fn resource_favorite_remove(project: SourceProjectRef) -> Result<()> {
    theseus::resources::favorites::remove(&project).await?;
    Ok(())
}

#[tauri::command]
pub async fn resource_locate(
    source: SourceId,
    locator: String,
    request_id: RequestId,
    cache_policy: Option<CachePolicy>,
) -> Result<ProjectDocument> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .locate_with_policy(
            &source,
            locator,
            &cancel,
            cache_policy.unwrap_or(CachePolicy::UseFresh),
        )
        .await?)
}

#[tauri::command]
pub async fn resource_project_document(
    project: SourceProjectRef,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<ProjectDocument> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .project_document(&project, locator, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_release_documents(
    source: SourceId,
    release_ids: Vec<SourceReleaseId>,
    project_id: Option<SourceProjectId>,
    request_id: RequestId,
    cache_policy: Option<CachePolicy>,
) -> Result<Vec<ReleaseDocument>> {
    let (_guard, cancel) = request(&request_id)?;
    let service = service().await?;
    let queries = release_ids
        .into_iter()
        .map(|release_id| ProviderQuery::Release {
            project_id: project_id.clone(),
            release_id,
            slug: None,
        })
        .collect();
    service
        .presentation_many_with_policy(
            &source,
            queries,
            &cancel,
            cache_policy.unwrap_or(CachePolicy::UseFresh),
        )
        .await
        .into_iter()
        .filter_map(|result| match result {
            Ok(QueryOutcome {
                value: ProviderValue::Release(document),
                ..
            }) => Some(Ok(document)),
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                ..
            }) => None,
            Err(error) => Some(Err(error.into())),
            _ => Some(Err(ResourceError::InvalidResponse(
                "expected release document".into(),
            )
            .into())),
        })
        .collect()
}

#[tauri::command]
pub async fn resource_project_documents(
    source: SourceId,
    project_ids: Vec<SourceProjectId>,
    request_id: RequestId,
    cache_policy: Option<CachePolicy>,
) -> Result<Vec<ProjectDocument>> {
    let (_guard, cancel) = request(&request_id)?;
    let queries = project_ids
        .into_iter()
        .map(|id| ProviderQuery::Project { id, slug: None })
        .collect();
    service()
        .await?
        .presentation_many_with_policy(
            &source,
            queries,
            &cancel,
            cache_policy.unwrap_or(CachePolicy::UseFresh),
        )
        .await
        .into_iter()
        .filter_map(|result| match result {
            Ok(QueryOutcome {
                value: ProviderValue::Project(document),
                ..
            }) => Some(Ok(document)),
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                ..
            }) => None,
            Err(error) => Some(Err(error.into())),
            _ => Some(Err(ResourceError::InvalidResponse(
                "expected project document".into(),
            )
            .into())),
        })
        .collect()
}

#[tauri::command]
pub async fn resource_project_protocol(
    locators: Vec<String>,
    protocol_version: String,
    request_id: RequestId,
    cache_policy: Option<CachePolicy>,
) -> Result<Vec<ProjectDocument>> {
    let (_guard, cancel) = request(&request_id)?;
    let queries = locators
        .into_iter()
        .map(|locator| ProviderQuery::ProjectProtocol {
            locator,
            version: protocol_version.clone(),
        })
        .collect();
    service()
        .await?
        .presentation_many_with_policy(
            &SourceId::modrinth(),
            queries,
            &cancel,
            cache_policy.unwrap_or(CachePolicy::UseFresh),
        )
        .await
        .into_iter()
        .filter_map(|result| match result {
            Ok(QueryOutcome {
                value: ProviderValue::Project(document),
                ..
            }) => Some(Ok(document)),
            Ok(QueryOutcome {
                value: ProviderValue::NotFound,
                ..
            }) => None,
            Err(error) => Some(Err(error.into())),
            _ => Some(Err(ResourceError::InvalidResponse(
                "expected project protocol document".into(),
            )
            .into())),
        })
        .collect()
}

#[tauri::command]
pub async fn resource_sources() -> Result<Vec<ResourceSource>> {
    Ok(service().await?.sources())
}

#[tauri::command]
pub async fn resource_search(
    query: ResourceSearchQuery,
    cursor: Option<String>,
    request_id: RequestId,
) -> Result<ResourcePage<ResourceSearchResult>> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .search(query, cursor, request_id, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_get(
    reference: ResourceRef,
    source: Option<SourceId>,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<Resource> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .get(&reference, source, locator, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_get_description(
    reference: ResourceRef,
    source: SourceId,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<ResourceText> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .description(&reference, &source, locator, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_list_releases(
    reference: ResourceRef,
    target: ResourceTarget,
    cursor: Option<String>,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<ResourcePage<ResourceRelease>> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .releases(&reference, target, cursor, request_id, locator, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_get_release(
    reference: SourceReleaseRef,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<ResourceRelease> {
    let (_guard, cancel) = request(&request_id)?;
    Ok(service()
        .await?
        .release(&reference, locator, &cancel)
        .await?)
}

#[tauri::command]
pub async fn resource_get_changelog(
    release_ref: SourceReleaseRef,
    locator: Option<String>,
    request_id: RequestId,
) -> Result<ResourceChangelog> {
    let (_guard, cancel) = request(&request_id)?;
    let service = service().await?;
    let text = service.changelog(&release_ref, locator, &cancel).await?;
    let published_at = match service
        .query(
            &release_ref.project.source,
            ProviderQuery::Release {
                project_id: Some(release_ref.project.project_id.clone()),
                release_id: release_ref.release_id.clone(),
                slug: None,
            },
            CachePolicy::OnlyCached,
            &cancel,
        )
        .await
    {
        Ok(QueryOutcome {
            value: ProviderValue::Release(document),
            ..
        }) => document.release.published_at,
        _ => None,
    };
    Ok(ResourceChangelog {
        release: release_ref,
        published_at,
        text,
    })
}

#[tauri::command]
pub async fn resource_list_changelogs(
    resource_ref: ResourceRef,
    cursor: Option<String>,
    request_id: RequestId,
) -> Result<ResourcePage<ResourceChangelog>> {
    let (_guard, cancel) = request(&request_id)?;
    let service = service().await?;
    let page = service
        .releases(
            &resource_ref,
            ResourceTarget {
                game_versions: vec![],
                loaders: vec![],
                channels: vec![],
            },
            cursor,
            request_id.clone(),
            None,
            &cancel,
        )
        .await?;
    let mut entries = Vec::new();
    let mut failures = page.source_failures;
    for release in page.entries {
        match service.changelog(&release.reference, None, &cancel).await {
            Ok(text) => entries.push(ResourceChangelog {
                release: release.reference,
                published_at: release.published_at,
                text,
            }),
            Err(ResourceError::Canceled) => {
                return Err(ResourceError::Canceled.into());
            }
            Err(error) => failures.push(ResourceSourceFailure {
                source: release.reference.project.source,
                code: ResourceIssueCode::SourceUnavailable,
                message: error.to_string(),
                retryable: true,
            }),
        }
    }
    Ok(ResourcePage {
        request_id,
        entries,
        source_failures: failures,
        has_more: page.has_more,
        cursor: page.cursor,
        total: page.total,
        exhausted_sources: page.exhausted_sources,
    })
}

#[tauri::command]
pub fn resource_cancel(request_id: RequestId) {
    if let Some(token) = REQUESTS.get(&request_id) {
        token.cancel();
    } else {
        EARLY_CANCELLATIONS.retain(|_, time| {
            time.elapsed() < std::time::Duration::from_secs(30)
        });
        if EARLY_CANCELLATIONS.len() < 256 {
            EARLY_CANCELLATIONS
                .insert(request_id.clone(), std::time::Instant::now());
        }
        if let Some(token) = REQUESTS.get(&request_id) {
            token.cancel();
        }
    }
}

#[tauri::command]
pub async fn resource_preferred_source() -> Result<SourceId> {
    Ok(service().await?.preferred_source().await?)
}

#[tauri::command]
pub async fn resource_set_preferred_source(source: SourceId) -> Result<()> {
    let service = service().await?;
    if !service
        .sources()
        .iter()
        .any(|candidate| candidate.id == source)
    {
        return Err(
            ResourceError::InvalidRequest("unknown source".into()).into()
        );
    }
    service
        .store()
        .ok_or_else(|| {
            ResourceError::Database("resource store unavailable".into())
        })?
        .set_preferred_source(&source)
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn resource_cache_info() -> Result<ResourceCacheInfo> {
    Ok(service().await?.cache().info().await?)
}

#[tauri::command]
pub async fn resource_cache_clear() -> Result<()> {
    service().await?.cache().clear().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_cancellations_expire_before_request_ids_are_reused() {
        let recent = RequestId::new("recent-cancellation-test").unwrap();
        resource_cancel(recent.clone());
        let (guard, token) = request(&recent).unwrap();
        assert!(token.is_cancelled());
        drop(guard);

        let expired = RequestId::new("expired-cancellation-test").unwrap();
        EARLY_CANCELLATIONS.insert(
            expired.clone(),
            std::time::Instant::now() - std::time::Duration::from_secs(31),
        );
        let (guard, token) = request(&expired).unwrap();
        assert!(!token.is_cancelled());
        assert!(!EARLY_CANCELLATIONS.contains_key(&expired));
        drop(guard);
    }
}
