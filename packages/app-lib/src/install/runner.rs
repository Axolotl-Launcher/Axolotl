use super::events::{InstallProgressReporter, emit_install_job};
use super::model::{
    InstallCleanup, InstallContentBatchItem, InstallContinuationState,
    InstallErrorContext, InstallErrorView, InstallJavaStep, InstallJobDisplay,
    InstallJobEventKind, InstallJobSnapshot, InstallJobState, InstallJobStatus,
    InstallPauseReason, InstallPhaseDetails, InstallPhaseId,
    InstallPostInstallEdit, InstallProgress, InstallRequest,
    InstallRollbackState, InstallTarget, InstanceUpgradeCompatibilityWarning,
    InstanceUpgradeDisplayNames, InstanceUpgradeExecution,
    InstanceUpgradeExternalChange, InstanceUpgradeExternalChangeKind,
    InstanceUpgradeResult, InstanceUpgradeWatchBaseline, SharedUpgradeMode,
    initial_phase_for_request,
};
use super::{diagnostics, recovery, store};
use crate::ErrorKind;
use crate::api::pack::install_from::{
    CreatePackLocation, generate_pack_from_file,
    generate_pack_from_version_id_with_reporter, get_instance_from_pack,
};
use crate::api::pack::install_mrpack::{
    MrpackInstallOutcome, install_zipped_mrpack_files_with_reporter,
    related_file_paths,
};
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::instances::adapters::sqlite::content_sets;
use crate::state::store::Collection as _;
use crate::state::store::content_update_jobs::{
    ContentUpdateJobRecord, ContentUpdateJobs,
};
use crate::state::store::install_jobs::{
    InstallJobRecord as StoredJob, InstallJobs,
};
use crate::state::store::resource_operation_plans::ResourceOperationPlans;
use crate::state::{
    ContentProvider, ContentProviderRef, InstanceInstallStage, InstanceLink,
    InstanceUpgradeAction, InstanceUpgradeDependencyChangeKind,
    LoaderComponent, LoaderComponentKind, LoaderComponentRole, ModLoader,
    State,
};
use crate::util::fetch::DownloadReason;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use uuid::Uuid;

mod adjunct;
mod admission;
mod content_change;
mod export;
mod init;
mod lifecycle;
mod pack;
mod request;
mod upgrade;

#[cfg(test)]
use adjunct::*;
#[cfg(test)]
use lifecycle::*;
#[cfg(test)]
use pack::*;
#[cfg(test)]
use upgrade::*;

pub(crate) use adjunct::{
    install_liteloader_adjunct_resolved, install_optifabric_file,
    resolve_optifabric_version, validate_loader_components,
};

enum InstallExecutionOutcome<T> {
    Completed(T),
    WaitingForUser(InstallPauseReason),
}

tokio::task_local! {
    static EXECUTING_JOB: Uuid;
}

pub(crate) async fn recovery_execution_slot<'a>(
    state: &'a State,
    job_id: Uuid,
) -> crate::Result<(
    Option<tokio::sync::OwnedMutexGuard<()>>,
    tokio::sync::SemaphorePermit<'a>,
)> {
    let record = store::get_required(job_id, state).await?;
    admission::execution_slot(state, &record).await
}

/// Serializes interactive content mutations with task admission.
pub(crate) async fn interactive_content_guard(
    state: &State,
    instance_id: &str,
) -> crate::Result<Option<tokio::sync::OwnedMutexGuard<()>>> {
    if EXECUTING_JOB.try_with(|_| ()).is_ok() {
        return Ok(None);
    }
    let guard = admission::lock(state, Some(instance_id)).await?;
    if store::list(false, state).await?.iter().any(|job| {
        (job.instance_id.as_deref() == Some(instance_id)
            || admission::request_instance(&job.state.request).as_deref()
                == Some(instance_id))
            && !job.status.is_finished()
    }) {
        return Err(ErrorKind::InputError(
            "Another task is changing this instance; retry when it finishes"
                .into(),
        )
        .into());
    }
    Ok(guard)
}

pub async fn create_instance(
    name: String,
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
    icon_path: Option<String>,
    link: InstanceLink,
) -> crate::Result<InstallJobSnapshot> {
    create_instance_with_adjuncts(
        name,
        game_version,
        loader,
        loader_version,
        Vec::new(),
        icon_path,
        link,
        None,
    )
    .await
}

pub async fn create_instance_with_adjuncts(
    name: String,
    game_version: String,
    loader: ModLoader,
    loader_version: Option<String>,
    adjuncts: Vec<crate::state::LoaderComponent>,
    icon_path: Option<String>,
    link: InstanceLink,
    game_dir_override: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::CreateInstance {
        name,
        game_version,
        loader,
        loader_version,
        adjuncts,
        icon_path,
        link,
        game_dir_override,
    })
    .await
}

pub async fn create_modpack_instance(
    location: CreatePackLocation,
    post_install_edit: Option<InstallPostInstallEdit>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::CreateModpackInstance {
        location,
        post_install_edit,
    })
    .await
}

pub async fn import_instance(
    launcher_type: crate::api::pack::import::ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
    symlink: bool,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ImportInstance {
        launcher_type,
        base_path,
        instance_folder,
        instance_path: None,
        symlink,
        game_version: None,
        loader: None,
        loader_version: None,
        game_dir_override: None,
    })
    .await
}

/// Like [`import_instance`] but with a pre-resolved filesystem path.
/// Used by the frontend when the path is already known from scanning,
/// avoiding redundant config/registry re-resolution.
pub async fn import_instance_with_path(
    launcher_type: crate::api::pack::import::ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
    instance_path: Option<String>,
    symlink: bool,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ImportInstance {
        launcher_type,
        base_path,
        instance_folder,
        instance_path,
        symlink,
        game_version: None,
        loader: None,
        loader_version: None,
        game_dir_override: None,
    })
    .await
}

pub async fn import_instance_with_plan(
    launcher_type: crate::api::pack::import::ImportLauncherType,
    base_path: PathBuf,
    instance_folder: String,
    instance_path: Option<String>,
    symlink: bool,
    game_version: Option<String>,
    loader: Option<crate::state::ModLoader>,
    loader_version: Option<String>,
    game_dir_override: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ImportInstance {
        launcher_type,
        base_path,
        instance_folder,
        instance_path,
        symlink,
        game_version,
        loader,
        loader_version,
        game_dir_override,
    })
    .await
}

pub async fn duplicate_instance(
    source_instance_id: String,
) -> crate::Result<InstallJobSnapshot> {
    // Directly associated instances own no files to copy: duplicating one
    // would clone the linked launcher's `.minecraft` into Axolotl.
    let state = State::get().await?;
    if let Some(metadata) =
        crate::state::get_instance(&source_instance_id, &state.pool).await?
        && metadata.instance.is_direct_linked()
    {
        return Err(crate::ErrorKind::InputError(format!(
            "\"{}\" is directly associated with an external launcher and \
             cannot be duplicated; its files are managed by that launcher",
            metadata.instance.name
        ))
        .into());
    }
    drop(state);

    start(InstallRequest::DuplicateInstance { source_instance_id }).await
}

pub async fn install_existing_instance(
    instance_id: String,
    force: bool,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::InstallExistingInstance { instance_id, force }).await
}

pub async fn upgrade_unmanaged_instance(
    instance_id: String,
    plan_id: String,
    execution: InstanceUpgradeExecution,
    create_full_backup: bool,
    shared_upgrade_mode: SharedUpgradeMode,
    display_names: InstanceUpgradeDisplayNames,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::UpgradeUnmanagedInstance {
        instance_id,
        plan_id,
        execution,
        create_full_backup,
        shared_upgrade_mode,
        display_names,
    })
    .await
}

pub async fn install_curseforge_world(
    request: crate::api::curseforge::CurseForgeWorldInstallRequest,
    display_title: String,
    display_icon: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::InstallCurseForgeWorld {
        request,
        display_title,
        display_icon,
    })
    .await
}

pub async fn install_content_batch(
    instance_id: String,
    items: Vec<InstallContentBatchItem>,
    display_title: String,
    display_icon: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::InstallContentBatch {
        instance_id,
        items,
        display_title,
        display_icon,
    })
    .await
}

pub async fn queue_resource_plan(
    plan_id: crate::resources::PlanId,
    revision: u32,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let instance = ResourceOperationPlans::instance_at(
        &state.data,
        plan_id.as_str(),
        revision,
    )
    .await?;
    let _admission = admission::lock(&state, Some(&instance)).await?;
    let _writer = state.acquire_install_db_permit().await?;
    let mut tx = state.pool.begin().await?;
    let existing =
        ResourceOperationPlans::job_at_in(&mut tx, plan_id.as_str(), revision)
            .await?;
    if let Some(existing) = existing {
        drop(tx);
        return store::get(Uuid::parse_str(&existing)?, &state)
            .await?
            .map(|record| record.snapshot())
            .ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "resource plan's job no longer exists".into(),
                )
                .into()
            });
    }
    let payload = ResourceOperationPlans::queueable_payload_in(
        &mut tx,
        plan_id.as_str(),
        revision,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "resource plan was revised or already queued".into(),
        )
    })?;
    let draft: crate::resources::plans::OperationDraft =
        serde_json::from_str(&payload)?;
    let plan = draft.plan;
    if plan.actions.is_empty() || plan.issues.iter().any(|issue| issue.blocking)
    {
        return Err(crate::ErrorKind::InputError(
            "resource plan has no executable changes".into(),
        )
        .into());
    }
    let _lock = state
        .lock_instance_content_exclusive(&plan.instance_id)
        .await;
    let current_revision = content_sets::get_applied_content_set_revision(
        &plan.instance_id,
        &mut *tx,
    )
    .await?
    .map(|(_, revision)| revision);
    if current_revision.and_then(|revision| u64::try_from(revision).ok())
        != Some(plan.instance_revision)
    {
        return Err(crate::ErrorKind::InputError(
            "instance changed after review".into(),
        )
        .into());
    }
    if store::has_unfinished_job_in_transaction(&plan.instance_id, &mut tx)
        .await?
    {
        return Err(crate::ErrorKind::InputError(
            "another task is changing this instance".into(),
        )
        .into());
    }
    let id = Uuid::new_v4();
    let mut job = InstallJobState::new(InstallRequest::InstallContentBatch {
        instance_id: plan.instance_id.clone(),
        items: vec![InstallContentBatchItem::FrozenResource {
            plan: plan.clone(),
        }],
        display_title: if plan.intent
            == crate::resources::ResourceOperationIntent::Install
        {
            "Install resources"
        } else {
            "Change resources"
        }
        .into(),
        display_icon: None,
    });
    job.display = Some(InstallJobDisplay {
        title: if plan.intent
            == crate::resources::ResourceOperationIntent::Install
        {
            "Install resources"
        } else {
            "Change resources"
        }
        .into(),
        icon: None,
    });
    let now = chrono::Utc::now().timestamp();
    InstallJobs::insert_in(
        &mut tx,
        &id.to_string(),
        &StoredJob {
            instance_id: Some(plan.instance_id.clone()),
            kind: job.request.kind().as_str().to_string(),
            status: InstallJobStatus::Queued.as_str().to_string(),
            state: serde_json::to_string(&job)?,
            created: now,
            modified: now,
            finished: None,
            dismissed: false,
        },
    )
    .await?;
    let updated = ResourceOperationPlans::queue_in(
        &mut tx,
        plan_id.as_str(),
        revision,
        &id.to_string(),
    )
    .await?;
    if updated != 1 {
        return Err(crate::ErrorKind::InputError(
            "resource plan changed concurrently".into(),
        )
        .into());
    }
    tx.commit().await?;
    drop(_lock);
    drop(_writer);
    let record = store::get(id, &state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError("resource job was not created".into())
    })?;
    tracing::info!(job_id = %id, plan_id = plan_id.as_str(), revision, instance_id = %plan.instance_id, actions = plan.actions.len(), "Resource execution queued");
    lifecycle::spawn_job(id);
    if let Err(error) = emit_install_job(&record.snapshot()).await {
        tracing::warn!(%error, "Could not emit queued resource job");
    }
    Ok(record.snapshot())
}

pub async fn queue_content_change(
    instance_id: String,
    intent: crate::install::ContentChangeIntent,
    display_title: String,
    display_icon: Option<String>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ChangeContent {
        instance_id,
        intent,
        display_title,
        display_icon,
    })
    .await
}

pub async fn download_java(
    vendor: String,
    version: u32,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::DownloadJava { vendor, version }).await
}

pub async fn install_pack_to_existing_instance(
    instance_id: String,
    location: CreatePackLocation,
    post_install_edit: Option<InstallPostInstallEdit>,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::InstallPackToExistingInstance {
        instance_id,
        location,
        post_install_edit,
    })
    .await
}

pub async fn update_managed_curseforge_modpack(
    instance_id: String,
    file_id: u32,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::UpdateManagedCurseForgeModpack {
        instance_id,
        file_id,
    })
    .await
}

pub async fn list_jobs(
    include_finished: bool,
) -> crate::Result<Vec<InstallJobSnapshot>> {
    let state = State::get().await?;
    let mut snapshots = Vec::new();
    for job in store::list(include_finished, &state).await? {
        let snapshot = job.snapshot();
        snapshots.push(
            InstallProgressReporter::overlay_snapshot(job.id, snapshot)
                .await
                .unwrap_or_else(|| job.snapshot()),
        );
    }
    Ok(snapshots)
}

pub async fn get_job(job_id: Uuid) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    let snapshot = job.snapshot();
    Ok(InstallProgressReporter::overlay_snapshot(job_id, snapshot)
        .await
        .unwrap_or_else(|| job.snapshot()))
}

pub async fn job_support_details(job_id: Uuid) -> crate::Result<String> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    diagnostics::build_job_support_details(&job, &state).await
}

pub async fn retry_job(job_id: Uuid) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let mut job = store::get_required(job_id, &state).await?;
    let instance = if job.state.retry_from_checkpoint {
        current_instance_id(&job.state)
    } else {
        admission::request_instance(&job.state.request)
    };
    let _admission = admission::lock(&state, instance.as_deref()).await?;
    job = store::get_required(job_id, &state).await?;
    if matches!(
        job.status,
        InstallJobStatus::Queued
            | InstallJobStatus::Running
            | InstallJobStatus::WaitingForUser
    ) {
        return Ok(job.snapshot());
    }
    let existing = match instance.as_deref() {
        Some(instance) => {
            admission::existing_for_instance(
                &state,
                instance,
                &job.state.request,
                Some(job_id),
            )
            .await?
        }
        None => None,
    };
    if let Some(existing) = existing {
        return Ok(existing);
    }

    if !matches!(
        job.status,
        InstallJobStatus::Failed | InstallJobStatus::Interrupted
    ) {
        return Err(crate::ErrorKind::InputError(
            "Only failed or interrupted install jobs can be retried"
                .to_string(),
        )
        .into());
    }

    let resume_checkpoint = prepare_pack_retry(&mut job.state, &state).await?;
    if !resume_checkpoint {
        job.state.target = job.state.request.target();
        job.state.cleanup = job.state.request.cleanup();
        job.state.rollback = None;
    }
    job.state.error = None;
    job.state.rollback_error = None;
    job.state.pause_reason = None;
    if !matches!(job.state.request, InstallRequest::ChangeContent { .. }) {
        job.state.continuation = None;
    }
    job.state.context = None;
    job.state.progress.phase = initial_phase_for_request(&job.state.request);
    job.state.progress.progress = None;
    job.state.progress.details = InstallPhaseDetails::Empty;
    job.state.progress.parallel = None;
    if !resume_checkpoint {
        init::prepare_initial_instance(&mut job.state, &state).await?;
    }
    job.state.record_event(InstallJobEventKind::JobQueued {
        kind: job.state.request.kind(),
    });

    let record = store::update_status(
        job_id,
        InstallJobStatus::Queued,
        &job.state,
        &state,
    )
    .await?;
    InstallProgressReporter::reset_job(job_id);
    lifecycle::emit_job_best_effort(&record.snapshot()).await;
    lifecycle::spawn_job(job_id);

    // The spawned job may already have progressed (or finished) by the time
    // the command returns; hand the caller the freshest stored state.
    Ok(store::get_required(job_id, &state).await?.snapshot())
}

async fn prepare_pack_retry(
    job: &mut InstallJobState,
    state: &State,
) -> crate::Result<bool> {
    if !job.retry_from_checkpoint {
        return Ok(false);
    }
    let Some(id) = current_instance_id(job) else {
        job.retry_from_checkpoint = false;
        return Ok(false);
    };
    let Some(instance) = crate::state::get_instance(&id, &state.pool).await?
    else {
        job.retry_from_checkpoint = false;
        return Ok(false);
    };
    if !job.is_new_pack_checkpoint()
        || instance.instance.install_stage == InstanceInstallStage::Installed
    {
        return Err(ErrorKind::InputError("This partial installation no longer matches its download task; start a new installation".into()).into());
    }
    if let InstallRequest::CreateModpackInstance {
        location: CreatePackLocation::FromFile { path },
        ..
    } = &job.request
    {
        let current =
            crate::api::pack::install_from::local_pack_retry_identity(path)
                .await?;
        if job.local_pack_retry_identity.as_ref() != Some(&current) {
            return Err(ErrorKind::InputError("The local modpack archive changed after installation started; select it again to start a new installation".into()).into());
        }
    }
    Ok(true)
}

pub async fn repair_cache_and_retry_job(
    job_id: Uuid,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let initial_job = store::get_required(job_id, &state).await?;
    let _ = validated_cache_repair_types(&initial_job)?;

    let operation_lock = state
        .install_job_operation_locks
        .entry(job_id)
        .or_default()
        .clone();
    let mut operation = operation_lock.lock().await;
    if operation.cache_repair_started {
        return Ok(store::get_required(job_id, &state).await?.snapshot());
    }

    let job = store::get_required(job_id, &state).await?;
    let cache_types = validated_cache_repair_types(&job)?;
    operation.cache_repair_started = true;
    if cache_types
        .iter()
        .any(|kind| crate::resources::protocol::owns(*kind))
        && let Err(error) =
            async { state.resources().await?.cache().clear().await }.await
    {
        operation.cache_repair_started = false;
        return Err(crate::resources::protocol::error(error));
    }

    if let Err(error) = crate::state::CachedEntry::purge_cache_types(
        &cache_types,
        state.cache_pool(),
    )
    .await
    {
        operation.cache_repair_started = false;
        return Err(crate::ErrorKind::OtherError(format!(
            "Project cache cleanup failed; retry was not started: {error}"
        ))
        .into());
    }

    retry_job(job_id).await.map_err(|error| {
        crate::ErrorKind::OtherError(format!(
            "Project cache was cleared, but retry could not be started: {error}"
        ))
        .into()
    })
}

fn validated_cache_repair_types(
    job: &store::InstallJobRecord,
) -> crate::Result<Vec<crate::state::CacheValueType>> {
    validated_cache_repair_types_for(job.status, job.state.error.as_ref())
}

fn validated_cache_repair_types_for(
    status: InstallJobStatus,
    error: Option<&InstallErrorView>,
) -> crate::Result<Vec<crate::state::CacheValueType>> {
    if !matches!(
        status,
        InstallJobStatus::Failed | InstallJobStatus::Interrupted
    ) {
        return Err(crate::ErrorKind::InputError(
            "Only failed or interrupted install jobs can repair cache"
                .to_string(),
        )
        .into());
    }
    let error = error.ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Install job has no cache repair error".to_string(),
        )
    })?;
    if error.code != "cache_repair_required" {
        return Err(crate::ErrorKind::InputError(
            "Install job does not require cache repair".to_string(),
        )
        .into());
    }
    let cache_types = error
        .context
        .as_ref()
        .map(|context| context.cache_types.as_slice())
        .unwrap_or_default();
    if cache_types.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Install job has no repairable cache types".to_string(),
        )
        .into());
    }

    let mut validated = Vec::new();
    for cache_type in cache_types {
        let cache_type =
            crate::state::CacheValueType::from_repairable_str(cache_type)
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Cache type is not repairable: {cache_type}"
                    ))
                })?;
        if !validated.contains(&cache_type) {
            validated.push(cache_type);
        }
    }
    Ok(validated)
}

pub async fn resume_job(job_id: Uuid) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    if job.status != InstallJobStatus::WaitingForUser {
        return Err(crate::ErrorKind::InputError(
            "Only install jobs waiting for user action can be resumed"
                .to_string(),
        )
        .into());
    }

    queue_waiting_job(job_id, job.state, &state).await
}

pub async fn skip_missing_content_and_resume_job(
    job_id: Uuid,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    if job.status != InstallJobStatus::WaitingForUser {
        return Err(crate::ErrorKind::InputError(
            "Only install jobs waiting for user action can skip missing content"
                .to_string(),
        )
        .into());
    }
    if matches!(
        job.state.request,
        InstallRequest::UpdateManagedCurseForgeModpack { .. }
    ) {
        return Err(crate::ErrorKind::InputError(
            "CurseForge modpack version updates cannot skip required manual downloads"
                .to_string(),
        )
        .into());
    }

    let mut current_missing_paths = job
        .snapshot()
        .items
        .into_iter()
        .filter(|item| {
            item.status == super::model::DownloadItemStatus::Failed
                || (item.status == super::model::DownloadItemStatus::Skipped
                    && item.manual_url.is_some())
        })
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let mut job_state = job.state;
    let InstallPauseReason::MissingRequiredContent { paths, .. } =
        job_state.pause_reason.as_ref().ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Install job has no missing content to skip".to_string(),
            )
        })?;
    if current_missing_paths.is_empty() {
        current_missing_paths = paths.clone();
    }
    if current_missing_paths.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Install job has no missing content to skip".to_string(),
        )
        .into());
    }
    job_state
        .skipped_missing_content_paths
        .extend(current_missing_paths);
    job_state.skipped_missing_content_paths.sort_unstable();
    job_state.skipped_missing_content_paths.dedup();

    queue_waiting_job(job_id, job_state, &state).await
}

async fn queue_waiting_job(
    job_id: Uuid,
    mut job_state: InstallJobState,
    state: &State,
) -> crate::Result<InstallJobSnapshot> {
    let instance = admission::request_instance(&job_state.request);
    let _admission = admission::lock(state, instance.as_deref()).await?;
    if let Some(existing) =
        admission::existing(state, &job_state.request, Some(job_id)).await?
    {
        return Ok(existing);
    }
    prepare_resumed_job(&mut job_state);
    let Some(record) = store::update_status_if(
        job_id,
        InstallJobStatus::WaitingForUser,
        InstallJobStatus::Queued,
        &job_state,
        state,
    )
    .await?
    else {
        return Err(crate::ErrorKind::InputError(
            "Install job is no longer waiting for user action".to_string(),
        )
        .into());
    };
    InstallProgressReporter::reset_job(job_id);
    lifecycle::emit_job_best_effort(&record.snapshot()).await;
    lifecycle::spawn_job(job_id);
    Ok(store::get_required(job_id, state).await?.snapshot())
}

fn prepare_resumed_job(job_state: &mut InstallJobState) {
    job_state.pause_reason = None;
    job_state.error = None;
    job_state.rollback_error = None;
    job_state.context = None;
    job_state.active_downloads.clear();
    job_state.record_event(InstallJobEventKind::JobQueued {
        kind: job_state.request.kind(),
    });
}

pub async fn retry_job_as_new(
    job_id: Uuid,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    if !matches!(
        job.status,
        InstallJobStatus::Failed
            | InstallJobStatus::Interrupted
            | InstallJobStatus::Canceled
    ) {
        return Err(crate::ErrorKind::InputError(
            "Only failed, interrupted, or canceled downloads can be retried"
                .to_string(),
        )
        .into());
    }
    let new_job = start(job.state.request).await?;
    // The spawned job may already have progressed (or finished) by the time
    // the command returns; hand the caller the freshest stored state.
    Ok(store::get_required(new_job.job_id, &state)
        .await?
        .snapshot())
}

pub async fn cancel_job(job_id: Uuid) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    cancel_job_with_state(job_id, state).await
}

pub(super) async fn cancel_job_with_state(
    job_id: Uuid,
    state: std::sync::Arc<State>,
) -> crate::Result<InstallJobSnapshot> {
    let started = std::time::Instant::now();
    let mut signaled = false;
    if let Some(token) = state.install_job_cancellations.get(&job_id) {
        token.value().cancel();
        signaled = true;
    }
    loop {
        let mut job = store::get_required(job_id, &state).await?;
        tracing::info!(%job_id, status = job.status.as_str(), phase = ?job.state.progress.phase, signaled, "Install cancellation requested");
        match job.status {
            InstallJobStatus::Running => {
                // Cancellation must not wait behind SQLite. Stop transfers,
                // verification and database workers first; the durable status
                // transition can then wait for the current writer to finish.
                if let Some(token) =
                    state.install_job_cancellations.get(&job_id)
                {
                    token.value().cancel();
                    signaled = true;
                }
                // Preserve the newest runtime events in the canceling
                // checkpoint instead of reverting to the last database copy.
                let checkpoint = async {
                    let live_reporter =
                        InstallProgressReporter::new(job_id, job.state.clone());
                    if let Ok(live_state) = live_reporter.current_state().await
                    {
                        job.state = live_state;
                    }
                    store::update_status_if(
                        job_id,
                        InstallJobStatus::Running,
                        InstallJobStatus::Canceling,
                        &job.state,
                        &state,
                    )
                    .await
                };
                let record = if signaled {
                    match tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        checkpoint,
                    )
                    .await
                    {
                        Ok(result) => result?,
                        Err(_) => {
                            let current =
                                store::get_required(job_id, &state).await?;
                            tracing::warn!(%job_id, status = current.status.as_str(), phase = ?current.state.progress.phase,
                                elapsed_ms = started.elapsed().as_millis() as u64,
                                "Install cancellation signaled; checkpoint is still waiting for progress or database access");
                            return Ok(current.snapshot());
                        }
                    }
                } else {
                    tokio::time::timeout(std::time::Duration::from_secs(20), checkpoint)
                        .await
                        .map_err(|_| crate::ErrorKind::InputError("Timed out accepting install cancellation before its worker registered; retry cancellation".into()))??
                };
                let Some(record) = record else {
                    continue;
                };
                if let Err(error) = emit_install_job(&record.snapshot()).await {
                    tracing::warn!(%job_id, %error, "Could not emit accepted install cancellation");
                }
                tracing::info!(%job_id, status = record.status.as_str(), elapsed_ms = started.elapsed().as_millis() as u64, "Install cancellation accepted");
                schedule_cancel_cleanup(job_id, state.clone());
                return Ok(record.snapshot());
            }
            InstallJobStatus::Canceling | InstallJobStatus::Canceled => {
                tracing::info!(%job_id, status = job.status.as_str(), "Install cancellation already accepted");
                if job.status == InstallJobStatus::Canceling {
                    if let Some(token) =
                        state.install_job_cancellations.get(&job_id)
                    {
                        token.value().cancel();
                    }
                    schedule_cancel_cleanup(job_id, state.clone());
                }
                return Ok(job.snapshot());
            }
            InstallJobStatus::Queued | InstallJobStatus::WaitingForUser => {
                let expected = job.status;
                begin_canceling_job(&mut job.state);
                let Some(record) = store::update_status_if(
                    job_id,
                    expected,
                    InstallJobStatus::Canceling,
                    &job.state,
                    &state,
                )
                .await?
                else {
                    continue;
                };
                let snapshot = record.snapshot();
                if let Err(error) = emit_install_job(&snapshot).await {
                    tracing::warn!(%job_id, %error, "Could not emit accepted install cancellation");
                }
                tracing::info!(%job_id, status = record.status.as_str(), elapsed_ms = started.elapsed().as_millis() as u64, "Install cancellation accepted; cleanup continues in background");
                schedule_cancel_cleanup(job_id, state.clone());
                return Ok(snapshot);
            }
            _ => {
                tracing::info!(%job_id, status = job.status.as_str(), "Install cancellation found an already finished job");
                return Ok(job.snapshot());
            }
        }
    }
}

static CANCEL_CLEANUPS: std::sync::LazyLock<dashmap::DashMap<Uuid, bool>> =
    std::sync::LazyLock::new(dashmap::DashMap::new);

struct CancelCleanupRegistration {
    job_id: Uuid,
    active: bool,
}

impl CancelCleanupRegistration {
    fn request(job_id: Uuid) -> Option<Self> {
        match CANCEL_CLEANUPS.entry(job_id) {
            dashmap::mapref::entry::Entry::Occupied(mut entry) => {
                *entry.get_mut() = true;
                None
            }
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(false);
                Some(Self {
                    job_id,
                    active: true,
                })
            }
        }
    }

    fn retry_requested(&mut self) -> bool {
        match CANCEL_CLEANUPS.entry(self.job_id) {
            dashmap::mapref::entry::Entry::Occupied(mut entry)
                if *entry.get() =>
            {
                *entry.get_mut() = false;
                true
            }
            dashmap::mapref::entry::Entry::Occupied(entry) => {
                entry.remove();
                self.active = false;
                false
            }
            dashmap::mapref::entry::Entry::Vacant(_) => unreachable!(),
        }
    }
}

impl Drop for CancelCleanupRegistration {
    fn drop(&mut self) {
        if self.active {
            CANCEL_CLEANUPS.remove(&self.job_id);
        }
    }
}

fn schedule_cancel_cleanup(job_id: Uuid, state: std::sync::Arc<State>) {
    let Some(registration) = CancelCleanupRegistration::request(job_id) else {
        return;
    };
    tokio::spawn(async move {
        run_cancel_cleanup(registration, || {
            finish_canceling_job(job_id, &state)
        })
        .await;
    });
}

async fn run_cancel_cleanup<F, Fut>(
    mut registration: CancelCleanupRegistration,
    mut cleanup: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = crate::Result<()>>,
{
    loop {
        if let Err(error) = cleanup().await {
            let job_id = registration.job_id;
            tracing::error!(%job_id, %error, "Canceled install cleanup could not finish; durable canceling state remains recoverable");
        }
        // Consume a pending request or release ownership under the same entry
        // lock, so a request arriving during cleanup cannot be stranded.
        if !registration.retry_requested() {
            break;
        }
    }
}

async fn finish_canceling_job(
    job_id: Uuid,
    state: &State,
) -> crate::Result<()> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let initial = store::get_required(job_id, state).await?;
    let _execution = admission::execution_lock(state, &initial).await;
    let mut job = store::get_required(job_id, state).await?;
    if job.status != InstallJobStatus::Canceling {
        return Ok(());
    }
    if lifecycle::complete_committed_job(job_id, &mut job.state, state).await? {
        return Ok(());
    }
    if job.state.progress.phase != InstallPhaseId::RollingBack {
        begin_canceling_job(&mut job.state);
    }
    let started = std::time::Instant::now();
    tracing::info!(%job_id, phase = ?job.state.progress.phase, "Canceled install cleanup started");
    let cleanup_succeeded =
        match recovery::apply_cleanup(&mut job.state, state).await {
            Ok(()) => true,
            Err(error) => {
                job.state.rollback_error = Some(InstallErrorView::from_error(
                    "rollback_error",
                    InstallPhaseId::RollingBack,
                    &error,
                    None,
                ));
                job.state.record_event(InstallJobEventKind::RollbackFailed {
                    message: error.to_string(),
                });
                false
            }
        };
    recovery::finalize_rollback_state(&mut job.state, cleanup_succeeded);
    if cleanup_succeeded {
        clear_deleted_new_instance_id(&mut job.state);
    }
    let record = store::update_status_if(
        job_id,
        InstallJobStatus::Canceling,
        InstallJobStatus::Canceled,
        &job.state,
        state,
    )
    .await?;
    if let Some(record) = record {
        lifecycle::emit_job_best_effort(&record.snapshot()).await;
    }
    tracing::info!(%job_id, cleanup_succeeded, elapsed_ms = started.elapsed().as_millis() as u64, "Canceled install cleanup finished");
    Ok(())
}

fn begin_canceling_job(job_state: &mut InstallJobState) {
    job_state.retry_from_checkpoint = false;
    let canceled_phase = job_state.progress.phase;
    job_state.error = Some(InstallErrorView::from_message(
        "canceled",
        canceled_phase,
        "Install was canceled",
    ));
    job_state.pause_reason = None;
    job_state.record_event(InstallJobEventKind::JobCanceled {
        phase: canceled_phase,
    });
    job_state.progress.phase = InstallPhaseId::RollingBack;
    job_state.progress.progress = None;
    job_state.progress.details = InstallPhaseDetails::Empty;
    job_state.progress.parallel = None;
    job_state.record_event(InstallJobEventKind::RollbackStarted {
        cleanup: job_state.cleanup.clone(),
    });
}

pub async fn dismiss_job(job_id: Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    store::dismiss(job_id, &state).await
}

pub async fn clear_job_history() -> crate::Result<u64> {
    let state = State::get().await?;
    store::clear_finished(&state).await
}

pub async fn export_resource(
    plan: crate::resources::ResourceExportPlan,
    output_path: PathBuf,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ExportResource {
        plan,
        output_path,
        resolution: None,
    })
    .await
}

pub async fn resolve_export_resource(
    plan: crate::resources::ResourceExportPlan,
    checkpoint: crate::resources::export::job::ExportResolutionCheckpoint,
    output_path: PathBuf,
) -> crate::Result<InstallJobSnapshot> {
    start(InstallRequest::ExportResource {
        plan,
        output_path,
        resolution: Some(Box::new(checkpoint)),
    })
    .await
}

async fn start(request: InstallRequest) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let snapshot = enqueue(request, &state).await?;
    lifecycle::spawn_job(snapshot.job_id);
    if let Err(error) = emit_install_job(&snapshot).await {
        tracing::warn!(%error, "Could not emit queued install task");
    }
    Ok(snapshot)
}

async fn enqueue(
    request: InstallRequest,
    state: &State,
) -> crate::Result<InstallJobSnapshot> {
    enqueue_with_content_plan(request, None, state).await
}

async fn enqueue_with_content_plan(
    request: InstallRequest,
    content_plan: Option<(&str, u64)>,
    state: &State,
) -> crate::Result<InstallJobSnapshot> {
    let instance = admission::request_instance(&request);
    let _admission = admission::lock(state, instance.as_deref()).await?;
    if let Some((plan_id, _)) = content_plan
        && let Some(job) = content_plan_job(plan_id, state).await?
    {
        return Ok(job);
    }
    if let Some(existing) = admission::existing(state, &request, None).await? {
        if let Some((plan_id, _)) = content_plan {
            ContentUpdateJobs::insert_in(
                &state.pool,
                &plan_id.to_string(),
                &ContentUpdateJobRecord {
                    job_id: Some(existing.job_id.to_string()),
                },
            )
            .await?;
        }
        return Ok(existing);
    }
    let _content_lock = match (content_plan, instance.as_deref()) {
        (Some((_, revision)), Some(instance_id)) => {
            let lock = state
                .lock_instance_content_with_timeout(
                    instance_id,
                    std::time::Duration::from_secs(20),
                )
                .await?;
            let current = crate::resources::plans::instance_revision(
                &state.pool,
                instance_id,
            )
            .await?;
            if current != revision {
                return Err(ErrorKind::InputError(
                    "Instance changed after update review".into(),
                )
                .into());
            }
            Some(lock)
        }
        _ => None,
    };
    let id = Uuid::new_v4();
    let mut job_state = InstallJobState::new(request);
    if let Err(error) =
        init::prepare_initial_instance(&mut job_state, state).await
    {
        return Err(cleanup_failed_initial_install(
            &mut job_state,
            state,
            error,
        )
        .await);
    }
    let record = match store::insert_with_content_plan(
        id,
        &job_state,
        InstallJobStatus::Queued,
        content_plan.map(|(id, _)| id),
        state,
    )
    .await
    {
        Ok(record) => record,
        Err(error) => {
            return Err(cleanup_failed_initial_install(
                &mut job_state,
                state,
                error,
            )
            .await);
        }
    };
    Ok(record.snapshot())
}

pub(crate) async fn content_plan_job(
    plan_id: &str,
    state: &State,
) -> crate::Result<Option<InstallJobSnapshot>> {
    let saved =
        ContentUpdateJobs::get(&state.data, &plan_id.to_string()).await?;
    match saved {
        None => Ok(None),
        Some(ContentUpdateJobRecord { job_id: Some(id) }) => Ok(Some(
            store::get_required(Uuid::parse_str(&id)?, state)
                .await?
                .snapshot(),
        )),
        Some(ContentUpdateJobRecord { job_id: None }) => Err(ErrorKind::InputError("The accepted task was removed from history; create a new update plan".into()).into()),
    }
}

pub(crate) async fn queue_content_update_plan(
    plan: &crate::state::ContentUpdatePlan,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let action = plan.actions.first().ok_or_else(|| {
        ErrorKind::InputError("Pack plan has no update action".into())
    })?;
    let request = match action.provider {
        ContentProvider::Modrinth => {
            let metadata =
                crate::state::get_instance(&plan.instance_id, &state.pool)
                    .await?
                    .ok_or_else(|| {
                        ErrorKind::InputError("Unknown instance".into())
                    })?;
            let (project_id, post_install_edit) = match metadata.link {
                InstanceLink::ModrinthModpack { project_id, .. } => {
                    (project_id, None)
                }
                InstanceLink::ServerProjectModpack {
                    server_project_id,
                    content_project_id,
                    ..
                } => (
                    content_project_id.clone(),
                    Some(InstallPostInstallEdit {
                        name: Some(metadata.instance.name.clone()),
                        icon_path: Some(metadata.instance.icon_path.clone()),
                        link: Some(InstanceLink::ServerProjectModpack {
                            server_project_id,
                            content_project_id,
                            content_version_id: action
                                .target_release_id
                                .clone(),
                        }),
                    }),
                ),
                _ => {
                    return Err(ErrorKind::InputError(
                        "Instance is not linked to a managed Modrinth pack"
                            .into(),
                    )
                    .into());
                }
            };
            InstallRequest::InstallPackToExistingInstance {
                instance_id: plan.instance_id.clone(),
                location: CreatePackLocation::FromVersionId {
                    project_id,
                    version_id: action.target_release_id.clone(),
                    title: metadata.instance.name,
                    icon_url: None,
                },
                post_install_edit,
            }
        }
        ContentProvider::CurseForge => {
            InstallRequest::UpdateManagedCurseForgeModpack {
                instance_id: plan.instance_id.clone(),
                file_id: action.target_release_id.parse().map_err(|_| {
                    ErrorKind::InputError("Invalid CurseForge file ID".into())
                })?,
            }
        }
        _ => {
            return Err(ErrorKind::InputError(
                "Source does not support automatic pack updates".into(),
            )
            .into());
        }
    };
    let snapshot = enqueue_with_content_plan(
        request,
        Some((&plan.id, plan.revision)),
        &state,
    )
    .await?;
    lifecycle::spawn_job(snapshot.job_id);
    if let Err(error) = emit_install_job(&snapshot).await {
        tracing::warn!(%error, "Could not emit accepted pack update");
    }
    Ok(snapshot)
}

async fn cleanup_failed_initial_install(
    job_state: &mut InstallJobState,
    state: &State,
    error: crate::Error,
) -> crate::Error {
    match recovery::apply_cleanup(job_state, state).await {
        Ok(()) => error,
        Err(cleanup_error) => crate::ErrorKind::OtherError(format!(
            "Install initialization failed: {error}; cleanup also failed: {cleanup_error}"
        ))
        .into(),
    }
}
async fn apply_post_install_edit(
    instance_id: &str,
    edit: Option<InstallPostInstallEdit>,
) -> crate::Result<()> {
    let Some(edit) = edit else {
        return Ok(());
    };

    if edit.name.is_none() && edit.icon_path.is_none() && edit.link.is_none() {
        return Ok(());
    }

    crate::api::instance::edit(
        instance_id,
        crate::state::instances::commands::EditInstance {
            name: edit.name,
            icon_path: edit.icon_path,
            link: edit.link,
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

async fn remove_existing_pack_content(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    instance_id: &str,
) -> crate::Result<HashSet<String>> {
    let metadata = crate::state::instances::commands::get_instance_metadata(
        instance_id,
        &state.pool,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError("Unknown instance".to_string())
    })?;
    let (project_id, version_id) = match &metadata.link {
        InstanceLink::ModrinthModpack {
            project_id,
            version_id,
        } => (project_id.clone(), version_id.clone()),
        InstanceLink::ServerProjectModpack {
            content_project_id,
            content_version_id,
            ..
        } => (content_project_id.clone(), content_version_id.clone()),
        InstanceLink::ImportedModpack { .. } => {
            recovery::prepare_existing_content_rollback(
                job_id,
                job_state,
                state,
                Vec::new(),
            )
            .await?;
            return Ok(HashSet::new());
        }
        _ => return Ok(HashSet::new()),
    };

    let disabled_project_ids =
        crate::state::instances::commands::list_project_files(
            instance_id,
            state,
        )
        .await?
        .into_iter()
        .filter_map(|file| {
            (!file.enabled).then(|| {
                file.provider_refs
                    .iter()
                    .find_map(|provider| match provider {
                        ContentProviderRef::Modrinth { project_id, .. } => {
                            Some(project_id.to_string())
                        }
                        ContentProviderRef::CurseForge { .. } => None,
                        ContentProviderRef::McArchive { .. } => None,
                    })
            })?
        })
        .collect::<HashSet<_>>();
    let reporter = InstallProgressReporter::new(job_id, job_state.clone());
    let old_pack = generate_pack_from_version_id_with_reporter(
        project_id.clone(),
        version_id.clone(),
        metadata.instance.name.clone(),
        None,
        instance_id.to_string(),
        DownloadReason::Update,
        reporter,
    )
    .await?;

    let related_paths = related_file_paths(&old_pack.file).await?;
    recovery::prepare_existing_content_rollback(
        job_id,
        job_state,
        state,
        related_paths,
    )
    .await?;

    Ok(disabled_project_ids)
}

async fn restore_disabled_projects(
    instance_id: &str,
    disabled_project_ids: HashSet<String>,
    state: &State,
) -> crate::Result<()> {
    if disabled_project_ids.is_empty() {
        return Ok(());
    }

    for file in crate::state::instances::commands::list_project_files(
        instance_id,
        state,
    )
    .await?
    {
        let is_disabled_modrinth_project = file.provider_refs.iter().any(
            |provider| {
                matches!(
                    provider,
                    ContentProviderRef::Modrinth { project_id, .. }
                        if disabled_project_ids.contains(&project_id.to_string())
                )
            },
        );
        if file.enabled && is_disabled_modrinth_project {
            crate::state::instances::commands::toggle_disable_project(
                instance_id,
                &file.relative_path,
                Some(false),
                state,
            )
            .await?;
        }
    }

    Ok(())
}

async fn prepare_existing_rollback(
    job_state: &mut InstallJobState,
    state: &State,
    instance_id: &str,
) -> crate::Result<()> {
    if job_state.rollback.is_some() {
        return Ok(());
    }

    let instance = crate::state::get_instance(instance_id, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Unknown instance {instance_id}"
            ))
        })?;
    let install_stage = instance.instance.install_stage;
    set_display(
        job_state,
        instance.instance.name.clone(),
        instance.instance.icon_path.clone(),
    );
    job_state.rollback = Some(InstallRollbackState {
        instance,
        install_stage,
        content: None,
    });
    job_state.cleanup = InstallCleanup::RestoreExistingInstance {
        instance_id: instance_id.to_string(),
    };

    crate::state::instances::commands::set_instance_install_stage(
        instance_id,
        InstanceInstallStage::MinecraftInstalling,
        &state.pool,
    )
    .await?;
    emit_instance(instance_id, InstancePayloadType::Edited).await?;

    Ok(())
}

async fn update_progress(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
    phase: InstallPhaseId,
    details: InstallPhaseDetails,
) -> crate::Result<()> {
    job_state.set_progress(phase, None, details);
    let record = store::update_state(job_id, job_state, state).await?;
    lifecycle::emit_job_best_effort(&record.snapshot()).await;
    Ok(())
}

fn set_instance_id(job_state: &mut InstallJobState, instance_id: String) {
    job_state.target = match &job_state.target {
        InstallTarget::ExistingInstance { .. } => {
            InstallTarget::ExistingInstance {
                instance_id: instance_id.clone(),
            }
        }
        InstallTarget::NewInstance { .. } => InstallTarget::NewInstance {
            instance_id: Some(instance_id.clone()),
        },
    };
    job_state.cleanup = match &job_state.cleanup {
        InstallCleanup::RestoreExistingInstance { .. } => {
            InstallCleanup::RestoreExistingInstance { instance_id }
        }
        InstallCleanup::DeleteNewInstance { .. } => {
            InstallCleanup::DeleteNewInstance {
                instance_id: Some(instance_id),
            }
        }
        InstallCleanup::None => InstallCleanup::None,
    };
}

fn clear_deleted_new_instance_id(job_state: &mut InstallJobState) {
    if matches!(job_state.cleanup, InstallCleanup::DeleteNewInstance { .. }) {
        job_state.target = InstallTarget::NewInstance { instance_id: None };
        job_state.cleanup =
            InstallCleanup::DeleteNewInstance { instance_id: None };
    }
}

fn set_display(
    job_state: &mut InstallJobState,
    title: String,
    icon: Option<String>,
) {
    job_state.display = Some(InstallJobDisplay { title, icon });
}

fn install_error_view(
    phase: InstallPhaseId,
    error: &crate::Error,
    context: Option<InstallErrorContext>,
) -> InstallErrorView {
    let context = match error.raw.as_ref() {
        ErrorKind::CacheReadError {
            cache_type,
            sqlite_code,
            ..
        } => {
            let mut context = context.unwrap_or_else(|| {
                InstallErrorContext::new("read project metadata cache").build()
            });
            context.cache_types = vec![cache_type.clone()];
            context.sqlite_code = sqlite_code.clone();
            Some(context)
        }
        _ => context,
    };
    InstallErrorView::from_error(
        install_error_code(phase, error),
        phase,
        error,
        context,
    )
}

fn install_error_code(
    phase: InstallPhaseId,
    error: &crate::Error,
) -> &'static str {
    use InstallPhaseId::*;

    match error.raw.as_ref() {
        ErrorKind::ResourceError(error) => match error {
            crate::resources::ResourceError::Network { .. }
            | crate::resources::ResourceError::Unavailable => "network_error",
            crate::resources::ResourceError::Database(_) => "database_error",
            crate::resources::ResourceError::Cache(_) => {
                "cache_repair_required"
            }
            crate::resources::ResourceError::InvalidResponse(_) => {
                "parse_error"
            }
            _ => "export_error",
        },
        ErrorKind::CacheReadError { .. } => "cache_repair_required",
        ErrorKind::InputError(msg)
            if msg.starts_with("Unrecognized modpack format")
                && matches!(phase, ResolvingPack) =>
        {
            "unrecognized_format"
        }
        ErrorKind::InputError(msg)
            if matches!(phase, ExtractingOverrides)
                && msg.contains("MultiMC") =>
        {
            "pack_error"
        }
        ErrorKind::InputError(_) => match phase {
            Exporting => "export_error",
            PreparingInstance | CreatingBackup | Finalizing | Completed => {
                "instance_error"
            }
            ResolvingPack | DownloadingPackFile | ReadingPackManifest => {
                "pack_error"
            }
            DownloadingContent | StagingContent | ApplyingContent => {
                "content_error"
            }
            ExtractingOverrides => "path_error",
            PreparingJava => "java_error",
            DownloadingMinecraft => "instance_error",
            RollingBack => "rollback_error",
            ResolvingMinecraft
            | ResolvingLoader
            | RunningLoaderProcessors
            | UpdatingLoader
            | Verifying => "launcher_error",
        },
        ErrorKind::LauncherError(_) => match phase {
            RunningLoaderProcessors => "processor_error",
            PreparingJava => "java_error",
            ResolvingLoader => "loader_error",
            _ => "launcher_error",
        },
        ErrorKind::JREError(_) => "java_error",
        ErrorKind::NoValueFor(_) | ErrorKind::MetadataError(_) => match phase {
            ResolvingLoader => "loader_error",
            PreparingJava => "java_error",
            _ => "metadata_error",
        },
        ErrorKind::FetchError(_)
        | ErrorKind::NetworkError(_)
        | ErrorKind::HttpError { .. }
        | ErrorKind::ApiIsDownError(_) => "network_error",
        ErrorKind::DownloadResourceWaitTimeout(_) => "resource_wait_error",
        ErrorKind::Any(_)
            if matches!(
                phase,
                DownloadingPackFile
                    | DownloadingContent
                    | ResolvingMinecraft
                    | ResolvingLoader
                    | PreparingJava
                    | DownloadingMinecraft
            ) =>
        {
            "network_error"
        }
        ErrorKind::LabrinthError(_) => "api_error",
        ErrorKind::HashError(_, _) => "hash_error",
        ErrorKind::ZipError(_) => "archive_error",
        ErrorKind::DeserializationError(_) | ErrorKind::StripPrefixError(_) => {
            "path_error"
        }
        ErrorKind::FSError(_)
        | ErrorKind::IOError(_)
        | ErrorKind::StdIOError(_)
        | ErrorKind::UTFError(_) => "filesystem_error",
        ErrorKind::INIError(_) | ErrorKind::JSONError(_) => "parse_error",
        ErrorKind::Sqlx(_) | ErrorKind::SqlxMigrate(_) => "database_error",
        ErrorKind::JoinError(_)
        | ErrorKind::RecvError(_)
        | ErrorKind::AcquireError(_)
        | ErrorKind::EventError(_) => "internal_error",
        ErrorKind::OtherError(_) | ErrorKind::Any(_) => "internal_error",
        _ => "unknown_error",
    }
}

fn current_instance_id(job_state: &InstallJobState) -> Option<String> {
    match &job_state.target {
        InstallTarget::NewInstance { instance_id } => instance_id.clone(),
        InstallTarget::ExistingInstance { instance_id } => {
            Some(instance_id.clone())
        }
    }
}

pub(crate) const OPTIFABRIC_CURSEFORGE_PROJECT_ID: u32 = 322_385;

fn modpack_details(location: &CreatePackLocation) -> InstallPhaseDetails {
    match location {
        CreatePackLocation::FromVersionId {
            project_id,
            version_id,
            title,
            ..
        } => InstallPhaseDetails::Modpack {
            project_id: Some(project_id.clone()),
            version_id: Some(version_id.clone()),
            title: Some(title.clone()),
        },
        CreatePackLocation::FromFile { .. } => InstallPhaseDetails::Modpack {
            project_id: None,
            version_id: None,
            title: None,
        },
    }
}

#[cfg(test)]
mod tests;
