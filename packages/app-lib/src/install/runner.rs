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
    let _writer = state.acquire_install_db_permit().await?;
    let mut tx = state.pool.begin().await?;
    let existing: Option<String> = sqlx::query_scalar("SELECT job_id FROM resource_operation_plans WHERE id = ? AND revision = ?")
        .bind(plan_id.as_str()).bind(revision).fetch_optional(&mut *tx).await?.flatten();
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
    let payload: String = sqlx::query_scalar("SELECT payload FROM resource_operation_plans WHERE id = ? AND revision = ? AND queued = 0 AND job_id IS NULL")
        .bind(plan_id.as_str()).bind(revision).fetch_optional(&mut *tx).await?.ok_or_else(|| crate::ErrorKind::InputError("resource plan was revised or already queued".into()))?;
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
    let current_revision: i64 = sqlx::query_scalar("SELECT content.revision FROM instances instance JOIN instance_content_sets content ON content.id = instance.applied_content_set_id WHERE instance.id = ?").bind(&plan.instance_id).fetch_one(&mut *tx).await?;
    if u64::try_from(current_revision).ok() != Some(plan.instance_revision) {
        return Err(crate::ErrorKind::InputError(
            "instance changed after review".into(),
        )
        .into());
    }
    let overlapping: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM install_jobs WHERE instance_id = ? AND status IN ('queued', 'running', 'canceling', 'waiting_for_user')").bind(&plan.instance_id).fetch_one(&mut *tx).await?;
    if overlapping != 0 {
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
    sqlx::query("INSERT INTO install_jobs(id, instance_id, kind, status, state, created, modified, finished, dismissed) VALUES (?, ?, ?, 'queued', ?, ?, ?, NULL, 0)").bind(id.to_string()).bind(&plan.instance_id).bind(job.request.kind().as_str()).bind(serde_json::to_string(&job)?).bind(now).bind(now).execute(&mut *tx).await?;
    let updated = sqlx::query("UPDATE resource_operation_plans SET job_id = ?, queued = 1 WHERE id = ? AND revision = ? AND queued = 0 AND job_id IS NULL").bind(id.to_string()).bind(plan_id.as_str()).bind(revision).execute(&mut *tx).await?;
    if updated.rows_affected() != 1 {
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
    let state = State::get().await?;
    let _instance_lock = state
        .lock_instance_content_with_timeout(
            &instance_id,
            std::time::Duration::from_secs(20),
        )
        .await?;
    let has_overlapping_change =
        store::list(false, &state).await?.into_iter().any(|job| {
            matches!(
                job.status,
                InstallJobStatus::Queued
                    | InstallJobStatus::Running
                    | InstallJobStatus::Canceling
                    | InstallJobStatus::WaitingForUser
            ) && matches!(
                &job.state.request,
                InstallRequest::ChangeContent {
                    instance_id: active_instance_id,
                    intent: active_intent,
                    ..
                } if active_instance_id == &instance_id
                    && content_change_intents_overlap(active_intent, &intent)
            )
        });
    if has_overlapping_change {
        return Err(crate::ErrorKind::InputError(
            "An overlapping content change is already active for this instance"
                .to_string(),
        )
        .into());
    }
    start(InstallRequest::ChangeContent {
        instance_id,
        intent,
        display_title,
        display_icon,
    })
    .await
}

fn content_change_intents_overlap(
    left: &crate::install::ContentChangeIntent,
    right: &crate::install::ContentChangeIntent,
) -> bool {
    use crate::install::ContentChangeIntent;

    if matches!(left, ContentChangeIntent::UpdateAllUserAdded)
        || matches!(right, ContentChangeIntent::UpdateAllUserAdded)
    {
        return true;
    }

    fn ids(intent: &ContentChangeIntent) -> Vec<&str> {
        match intent {
            ContentChangeIntent::UpdateOne { content_id, .. }
            | ContentChangeIntent::SwitchVersion { content_id, .. } => {
                vec![content_id]
            }
            ContentChangeIntent::UpdateSelected { targets } => targets
                .iter()
                .map(|target| target.content_id.as_str())
                .collect(),
            ContentChangeIntent::UpdateAllUserAdded => Vec::new(),
        }
    }

    let left = ids(left).into_iter().collect::<HashSet<_>>();
    ids(right)
        .into_iter()
        .any(|content_id| left.contains(content_id))
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

    job.state.target = job.state.request.target();
    job.state.cleanup = job.state.request.cleanup();
    job.state.rollback = None;
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
    init::prepare_initial_instance(&mut job.state, &state).await?;
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
    emit_install_job(&record.snapshot()).await?;
    lifecycle::spawn_job(job_id);

    // The spawned job may already have progressed (or finished) by the time
    // the command returns; hand the caller the freshest stored state.
    Ok(store::get_required(job_id, &state).await?.snapshot())
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

    if let Err(error) =
        crate::state::CachedEntry::purge_cache_types(&cache_types, &state.pool)
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
    let Some(InstallPauseReason::MissingRequiredContent { paths, .. }) =
        job_state.pause_reason.as_ref()
    else {
        return Err(crate::ErrorKind::InputError(
            "Install job has no missing content to skip".to_string(),
        )
        .into());
    };
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

/// Record the user's optional-file selection for a paused modpack install
/// and queue it again. `skipped_paths` are the install paths the user chose
/// to skip; every other optional file is installed.
pub async fn select_optional_content_and_resume_job(
    job_id: Uuid,
    skipped_paths: Vec<String>,
) -> crate::Result<InstallJobSnapshot> {
    let state = State::get().await?;
    let job = store::get_required(job_id, &state).await?;
    if job.status != InstallJobStatus::WaitingForUser {
        return Err(crate::ErrorKind::InputError(
            "Only install jobs waiting for user action can select optional content"
                .to_string(),
        )
        .into());
    }
    let mut job_state = job.state;
    if !matches!(
        job_state.pause_reason,
        Some(InstallPauseReason::OptionalContentSelection { .. })
    ) {
        return Err(crate::ErrorKind::InputError(
            "Install job is not waiting for an optional content selection"
                .to_string(),
        )
        .into());
    }
    let known_paths: std::collections::HashSet<&str> = job_state
        .optional_content
        .as_ref()
        .map(|content| {
            content.files.iter().map(|f| f.item_id.as_str()).collect()
        })
        .unwrap_or_default();
    let mut skipped = skipped_paths;
    skipped.retain(|path| known_paths.contains(path.as_str()));
    skipped.sort_unstable();
    skipped.dedup();
    job_state.skipped_optional_content_paths = skipped;
    job_state.optional_content_decided = true;
    queue_waiting_job(job_id, job_state, &state).await
}

async fn queue_waiting_job(
    job_id: Uuid,
    mut job_state: InstallJobState,
    state: &State,
) -> crate::Result<InstallJobSnapshot> {
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
    emit_install_job(&record.snapshot()).await?;
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
    if let Some(token) = state.install_job_cancellations.get(&job_id) {
        token.value().cancel();
    }
    let mut job = loop {
        let mut job = store::get_required(job_id, &state).await?;
        match job.status {
            InstallJobStatus::Running => {
                // Cancellation must not wait behind SQLite. Stop transfers,
                // verification and database workers first; the durable status
                // transition can then wait for the current writer to finish.
                if let Some(token) =
                    state.install_job_cancellations.get(&job_id)
                {
                    token.value().cancel();
                }
                // Preserve the newest runtime events in the canceling
                // checkpoint instead of reverting to the last database copy.
                let live_reporter =
                    InstallProgressReporter::new(job_id, job.state.clone());
                if let Ok(live_state) = live_reporter.current_state().await {
                    job.state = live_state;
                }
                let Some(record) = store::update_status_if(
                    job_id,
                    InstallJobStatus::Running,
                    InstallJobStatus::Canceling,
                    &job.state,
                    &state,
                )
                .await?
                else {
                    continue;
                };
                emit_install_job(&record.snapshot()).await?;
                return Ok(record.snapshot());
            }
            InstallJobStatus::Canceling | InstallJobStatus::Canceled => {
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
                emit_install_job(&record.snapshot()).await?;
                break record;
            }
            _ => {
                return Err(crate::ErrorKind::InputError(
                    "Only queued, running, or waiting install jobs can be canceled"
                        .to_string(),
                )
                .into());
            }
        }
    };

    let cleanup_succeeded =
        match recovery::apply_cleanup(&mut job.state, &state).await {
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
    let record = store::update_status(
        job_id,
        InstallJobStatus::Canceled,
        &job.state,
        &state,
    )
    .await?;
    emit_install_job(&record.snapshot()).await?;

    Ok(record.snapshot())
}

fn begin_canceling_job(job_state: &mut InstallJobState) {
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
    let id = Uuid::new_v4();
    let mut job_state = InstallJobState::new(request);
    init::prepare_initial_instance(&mut job_state, &state).await?;
    let record =
        match store::insert(id, &job_state, InstallJobStatus::Queued, &state)
            .await
        {
            Ok(record) => record,
            Err(error) => {
                return Err(cleanup_failed_initial_install(
                    &mut job_state,
                    &state,
                    error,
                )
                .await);
            }
        };
    emit_install_job(&record.snapshot()).await?;
    lifecycle::spawn_job(id);
    Ok(record.snapshot())
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
    emit_install_job(&record.snapshot()).await?;
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
