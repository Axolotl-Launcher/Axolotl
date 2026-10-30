use super::model::{
    InstallJobKind, InstallJobSnapshot, InstallJobState, InstallJobStatus,
};
use crate::state::store::content_update_jobs::{
    ContentUpdateJobRecord, ContentUpdateJobs,
};
use crate::state::store::install_jobs::{
    InstallJobRecord as StoredJob, InstallJobScope, InstallJobs, JobFinish,
};
use crate::state::store::{Atom, Collection as _, atom, names_a_missing_row};
use crate::state::{InstanceInstallStage, State};
use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

fn same_execution(
    stored: &str,
    incoming: &InstallJobState,
) -> crate::Result<bool> {
    fn queued(state: &InstallJobState) -> Option<chrono::DateTime<Utc>> {
        state.events.iter().rev().find_map(|event| {
            matches!(
                event.kind,
                super::model::InstallJobEventKind::JobQueued { .. }
            )
            .then_some(event.at)
        })
    }
    let stored: InstallJobState = serde_json::from_str(stored)?;
    Ok(queued(&stored) == queued(incoming))
}

#[derive(Clone, Debug)]
pub struct InstallJobRecord {
    pub id: Uuid,
    pub instance_id: Option<String>,
    pub kind: InstallJobKind,
    pub status: InstallJobStatus,
    pub state: InstallJobState,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub finished: Option<DateTime<Utc>>,
    pub dismissed: bool,
}

impl InstallJobRecord {
    pub fn snapshot(&self) -> InstallJobSnapshot {
        let summary = self.state.download_summary();
        let items = self.state.download_items();
        let recorded_instance_id = instance_id(&self.state);
        let instance_deleted = self.state.instance_deleted()
            || (self.status == InstallJobStatus::Succeeded
                && self.instance_id.is_none()
                && recorded_instance_id.is_some());
        InstallJobSnapshot {
            job_id: self.id,
            instance_id: self.instance_id.clone().or(recorded_instance_id),
            source_instance_id: self.state.source_instance_id(),
            instance_deleted,
            kind: self.kind,
            status: self.status,
            execution_mode: self.state.execution_mode(self.status),
            provider: self.state.provider(),
            target: self.state.target.clone(),
            phase: self.state.progress.phase,
            progress: self.state.progress.progress.clone(),
            details: self.state.progress.details.clone(),
            parallel: self.state.progress.parallel.clone(),
            display: self.state.display.clone(),
            error: self.state.error.clone(),
            rollback_error: self.state.rollback_error.clone(),
            pause_reason: self.state.pause_reason.clone(),
            upgrade_result: self.state.upgrade_result.clone(),
            content_change: self.state.content_change(),
            created: self.created,
            modified: self.modified,
            finished: self.finished,
            summary,
            items,
        }
    }
}

pub async fn insert(
    id: Uuid,
    state: &InstallJobState,
    status: InstallJobStatus,
    app_state: &State,
) -> crate::Result<InstallJobRecord> {
    insert_with_content_plan(id, state, status, None, app_state).await
}

pub(crate) async fn insert_with_content_plan(
    id: Uuid,
    state: &InstallJobState,
    status: InstallJobStatus,
    plan_id: Option<&str>,
    app_state: &State,
) -> crate::Result<InstallJobRecord> {
    let now = Utc::now().timestamp();
    let key = id.to_string();
    let row = StoredJob {
        instance_id: instance_id(state),
        kind: state.request.kind().as_str().to_string(),
        status: status.as_str().to_string(),
        state: serde_json::to_string(state)?,
        created: now,
        modified: now,
        finished: None,
        dismissed: false,
    };

    let mut unit = atom(&app_state.data).await?;
    write_job(&mut unit, &key, row).await?;
    if let Some(plan_id) = plan_id {
        let link = ContentUpdateJobRecord {
            job_id: Some(key.clone()),
        };
        ContentUpdateJobs::atom_set(
            &mut unit,
            &plan_id.to_string(),
            Some(&link),
        )
        .await?;
    }
    unit.commit().await?;

    get(id, app_state).await?.ok_or_else(|| {
        crate::ErrorKind::OtherError(format!(
            "Install job {id} was not inserted"
        ))
        .into()
    })
}

pub async fn get(
    id: Uuid,
    app_state: &State,
) -> crate::Result<Option<InstallJobRecord>> {
    let key = id.to_string();
    let row = InstallJobs::get(&app_state.data, &key).await?;
    row.map(|row| row_to_record(&key, row)).transpose()
}

/// Whether the instance still has a job that occupies it, read inside the
/// caller's transaction so the answer cannot go stale before its write lands.
pub(crate) async fn has_unfinished_job_in_transaction(
    instance_id: &str,
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> crate::Result<bool> {
    let statuses = InstallJobStatus::unfinished();
    InstallJobs::unfinished_in(&mut **tx, instance_id, &statuses).await
}

/// Whether any job still occupies an instance, or still holds a rollback.
pub(crate) async fn has_any_unfinished_job(
    pool: &sqlx::SqlitePool,
) -> crate::Result<bool> {
    let statuses = InstallJobStatus::unfinished();
    InstallJobs::unfinished_or_rollback(pool, &statuses).await
}

pub async fn list(
    include_finished: bool,
    app_state: &State,
) -> crate::Result<Vec<InstallJobRecord>> {
    let rows = if include_finished {
        InstallJobs::get_all(&app_state.data).await?
    } else {
        let mut unit = atom(&app_state.data).await?;
        let mut rows = Vec::new();
        for status in InstallJobStatus::ALL
            .into_iter()
            .filter(|status| !status.is_resolved())
        {
            let scope = InstallJobScope::Status(status.as_str().to_string());
            rows.extend(InstallJobs::atom_get_scoped(&mut unit, &scope).await?);
        }
        rows
    };
    let mut rows: Vec<_> =
        rows.into_iter().filter(|(_, row)| !row.dismissed).collect();
    rows.sort_unstable_by_key(|(_, row)| row.created);

    records(rows)
}

pub async fn list_interrupted_candidates(
    app_state: &State,
) -> crate::Result<Vec<InstallJobRecord>> {
    let mut unit = atom(&app_state.data).await?;
    let mut rows = Vec::new();
    for status in [
        InstallJobStatus::Queued,
        InstallJobStatus::Running,
        InstallJobStatus::Canceling,
    ] {
        let scope = InstallJobScope::Status(status.as_str().to_string());
        rows.extend(InstallJobs::atom_get_scoped(&mut unit, &scope).await?);
    }

    records(rows)
}

pub async fn update_state(
    id: Uuid,
    state: &InstallJobState,
    app_state: &State,
) -> crate::Result<InstallJobRecord> {
    let _db_permit =
        app_state
            .install_db_semaphore
            .acquire()
            .await
            .map_err(|_| {
                crate::ErrorKind::OtherError(
                    "install database semaphore closed".to_string(),
                )
            })?;
    let json = serde_json::to_string(state)?;
    let key = id.to_string();
    let modified = Utc::now().timestamp();

    let mut unit = atom(&app_state.data).await?;
    if let Some(mut row) = InstallJobs::atom_get(&mut unit, &key).await? {
        if !same_execution(&row.state, state)? {
            drop(unit);
            return get_required(id, app_state).await;
        }
        row.instance_id = instance_id(state);
        row.state = json;
        row.modified = modified;
        write_job(&mut unit, &key, row).await?;
    }
    unit.commit().await?;

    get_required(id, app_state).await
}

/// Persists a recoverable runtime checkpoint without reopening a terminal or
/// paused job. The status predicate makes a delayed checkpoint harmless when
/// job finalization wins the database semaphore first.
pub async fn checkpoint_running_state(
    id: Uuid,
    state: &InstallJobState,
    app_state: &State,
) -> crate::Result<Option<InstallJobRecord>> {
    let _db_permit =
        app_state
            .install_db_semaphore
            .acquire()
            .await
            .map_err(|_| {
                crate::ErrorKind::OtherError(
                    "install database semaphore closed".to_string(),
                )
            })?;
    let json = serde_json::to_string(state)?;
    let key = id.to_string();
    let modified = Utc::now().timestamp();

    let mut unit = atom(&app_state.data).await?;
    let Some(mut row) = InstallJobs::atom_get(&mut unit, &key).await? else {
        return Ok(None);
    };
    if row.status.as_str() != InstallJobStatus::Running.as_str()
        || !same_execution(&row.state, state)?
    {
        return Ok(None);
    }
    row.instance_id = instance_id(state);
    row.state = json;
    row.modified = modified;
    write_job(&mut unit, &key, row).await?;
    unit.commit().await?;

    Ok(Some(get_required(id, app_state).await?))
}

pub async fn update_status(
    id: Uuid,
    status: InstallJobStatus,
    state: &InstallJobState,
    app_state: &State,
) -> crate::Result<InstallJobRecord> {
    let _db_permit =
        app_state
            .install_db_semaphore
            .acquire()
            .await
            .map_err(|_| {
                crate::ErrorKind::OtherError(
                    "install database semaphore closed".to_string(),
                )
            })?;
    let now = Utc::now();
    let finished = status.is_finished().then_some(now.timestamp());
    let json = serde_json::to_string(state)?;
    let key = id.to_string();
    let modified = now.timestamp();

    let mut unit = atom(&app_state.data).await?;
    if let Some(mut row) = InstallJobs::atom_get(&mut unit, &key).await? {
        row.instance_id = instance_id(state);
        row.status = status.as_str().to_string();
        row.state = json;
        row.modified = modified;
        row.finished = finished;
        write_job(&mut unit, &key, row).await?;
    }
    unit.commit().await?;

    get_required(id, app_state).await
}

pub async fn update_status_if(
    id: Uuid,
    expected: InstallJobStatus,
    status: InstallJobStatus,
    state: &InstallJobState,
    app_state: &State,
) -> crate::Result<Option<InstallJobRecord>> {
    let _db_permit =
        app_state
            .install_db_semaphore
            .acquire()
            .await
            .map_err(|_| {
                crate::ErrorKind::OtherError(
                    "install database semaphore closed".to_string(),
                )
            })?;
    let now = Utc::now();
    let finished = status.is_finished().then_some(now.timestamp());
    let json = serde_json::to_string(state)?;
    let key = id.to_string();
    let modified = now.timestamp();

    let mut unit = atom(&app_state.data).await?;
    let updated = compare_and_swap_status(
        &mut unit,
        &key,
        expected,
        status,
        instance_id(state),
        &json,
        modified,
        finished,
    )
    .await?;
    if !updated {
        return Ok(None);
    }
    unit.commit().await?;

    Ok(Some(get_required(id, app_state).await?))
}

/// Not on the collections yet: it writes the instance's install stage and the
/// upgrade notice in the job row's transaction, and both of those writers take
/// a pool.
pub async fn complete_running_job(
    id: Uuid,
    state: &InstallJobState,
    app_state: &State,
) -> crate::Result<Option<InstallJobRecord>> {
    let now = Utc::now();
    let modified = now.timestamp();
    let json = serde_json::to_string(state)?;
    let id_value = id.to_string();
    let instance_id = instance_id(state);
    let completes_instance_install_stage =
        state.request.completes_instance_install_stage();
    let resource_committed = crate::resources::plans::frozen_request_completed(
        &state.request,
        &app_state.data,
    )
    .await?
        || (matches!(
            state.request,
            super::model::InstallRequest::ExportResource { .. }
        ) && state.events.iter().any(|e| {
            matches!(
                e.kind,
                super::model::InstallJobEventKind::JobSucceeded { .. }
            )
        }));
    let mut transaction = app_state.pool.begin().await?;

    if completes_instance_install_stage
        && let Some(instance_id) = instance_id.as_deref()
    {
        let reached = crate::state::instances::adapters::sqlite::instance_rows::set_install_stage_in_transaction(
            instance_id,
            InstanceInstallStage::Installed,
            modified,
            &mut transaction,
        )
        .await?;
        if reached != 1 {
            return Err(crate::ErrorKind::InputError(format!(
                "Install target instance {instance_id} no longer exists"
            ))
            .into());
        }
    }

    let reached = InstallJobs::finish_in(
        &mut transaction,
        &JobFinish {
            key: &id_value,
            instance_id: instance_id.as_deref(),
            status: InstallJobStatus::Succeeded.as_str(),
            state: &json,
            modified,
            finished: now.timestamp(),
            running_status: InstallJobStatus::Running.as_str(),
            canceling_status: InstallJobStatus::Canceling.as_str(),
            accept_canceling: resource_committed,
        },
    )
    .await?;

    if reached != 1 {
        transaction.rollback().await?;
        return Ok(None);
    }
    if let super::model::InstallRequest::UpgradeUnmanagedInstance {
        execution,
        ..
    } = &state.request
        && let Some(upgrade_result) = state.upgrade_result.as_ref()
        && let Some(target_environment) =
            upgrade_result.target_environment.as_ref()
    {
        let notice = crate::state::InstancePostUpgradeNotice {
            instance_id: upgrade_result.target_instance_id.clone(),
            upgrade_job_id: id_value.clone(),
            target_game_version: target_environment.game_version.clone(),
            consecutive_clean_launches: 0,
            warnings: crate::state::instances::commands::post_upgrade_warnings_from_result(
                upgrade_result,
                execution,
            ),
        };
        tracing::debug!(
            target_instance_id = %notice.instance_id,
            upgrade_job_id = %notice.upgrade_job_id,
            target_game_version = %notice.target_game_version,
            structured_compatibility_warning_count = upgrade_result.compatibility_warning_details.len(),
            post_upgrade_warning_count = notice.warnings.len(),
            "Persisting completed unmanaged upgrade notice"
        );
        crate::state::instances::commands::replace_instance_post_upgrade_notice_on_connection(
            &notice,
            &mut transaction,
        )
        .await?;
    }
    transaction.commit().await?;

    if completes_instance_install_stage
        && let Some(instance_id) = instance_id.as_deref()
        && let Err(error) =
            crate::api::instance::synced_options::reconcile_instance_after_pack_update_with_state(
                instance_id,
                app_state,
            )
            .await
    {
        tracing::warn!(
            %instance_id,
            %error,
            "Instance installation completed, but its enabled synchronized options could not be applied"
        );
    }

    Ok(Some(get_required(id, app_state).await?))
}

pub async fn dismiss(id: Uuid, app_state: &State) -> crate::Result<()> {
    let key = id.to_string();
    let modified = Utc::now().timestamp();

    let mut unit = atom(&app_state.data).await?;
    if let Some(mut row) = InstallJobs::atom_get(&mut unit, &key).await? {
        row.dismissed = true;
        row.modified = modified;
        InstallJobs::atom_set(&mut unit, &key, Some(&row)).await?;
    }
    unit.commit().await
}

pub async fn clear_finished(app_state: &State) -> crate::Result<u64> {
    let modified = Utc::now().timestamp();
    let mut cleared = 0u64;

    let mut unit = atom(&app_state.data).await?;
    for status in [
        InstallJobStatus::Succeeded,
        InstallJobStatus::Failed,
        InstallJobStatus::Interrupted,
        InstallJobStatus::Canceled,
    ] {
        let scope = InstallJobScope::Status(status.as_str().to_string());
        let finished = InstallJobs::atom_get_scoped(&mut unit, &scope).await?;
        for (key, mut row) in finished {
            if row.dismissed {
                continue;
            }
            row.dismissed = true;
            row.modified = modified;
            InstallJobs::atom_set(&mut unit, &key, Some(&row)).await?;
            cleared += 1;
        }
    }
    unit.commit().await?;

    Ok(cleared)
}

pub async fn mark_instance_deleted(
    instance_id: &str,
    app_state: &State,
) -> crate::Result<Vec<InstallJobRecord>> {
    let mut updated = Vec::new();
    let scope = InstallJobScope::Instance(instance_id.to_string());
    for (key, row) in InstallJobs::get_scoped(&app_state.data, &scope).await? {
        if row.dismissed {
            continue;
        }
        let mut record = row_to_record(&key, row)?;
        if record.state.instance_deleted() {
            updated.push(record);
            continue;
        }
        record.state.record_event(
            super::model::InstallJobEventKind::TargetInstanceDeleted {
                instance_id: instance_id.to_string(),
            },
        );
        updated.push(update_state(record.id, &record.state, app_state).await?);
    }
    Ok(updated)
}

pub async fn get_required(
    id: Uuid,
    app_state: &State,
) -> crate::Result<InstallJobRecord> {
    get(id, app_state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError(format!("Unknown install job {id}")).into()
    })
}

fn records(
    rows: Vec<(String, StoredJob)>,
) -> crate::Result<Vec<InstallJobRecord>> {
    rows.into_iter()
        .map(|(id, row)| row_to_record(&id, row))
        .collect()
}

async fn compare_and_swap_status(
    unit: &mut Atom,
    key: &str,
    expected: InstallJobStatus,
    status: InstallJobStatus,
    instance_id: Option<String>,
    state_json: &str,
    modified: i64,
    finished: Option<i64>,
) -> crate::Result<bool> {
    let stored = InstallJobs::atom_get(unit, &key.to_string()).await?;
    let Some(mut row) = stored else {
        return Ok(false);
    };
    if row.status.as_str() != expected.as_str() {
        return Ok(false);
    }
    row.instance_id = instance_id;
    row.status = status.as_str().to_string();
    row.state = state_json.to_string();
    row.modified = modified;
    row.finished = finished;
    write_job(unit, key, row).await?;
    Ok(true)
}

/// Writes `row` back under `key`, dropping its instance reference when the
/// database refuses it: the statement this replaced stored a job's instance
/// only while that instance still existed.
/// Writes the job, dropping the instance it names only when the table refused
/// that reference: the instance may have been deleted while the install ran, and
/// the job is still worth keeping without it.
async fn write_job(
    unit: &mut Atom,
    key: &str,
    mut row: StoredJob,
) -> crate::Result<()> {
    let key = key.to_string();
    match InstallJobs::atom_set(unit, &key, Some(&row)).await {
        Ok(()) => Ok(()),
        Err(error)
            if row.instance_id.is_some() && names_a_missing_row(&error) =>
        {
            row.instance_id = None;
            InstallJobs::atom_set(unit, &key, Some(&row)).await
        }
        Err(error) => Err(error),
    }
}

fn row_to_record(id: &str, row: StoredJob) -> crate::Result<InstallJobRecord> {
    Ok(InstallJobRecord {
        id: Uuid::parse_str(id).map_err(|err| {
            crate::ErrorKind::InputError(format!(
                "Invalid install job id {id}: {err}"
            ))
        })?,
        instance_id: row.instance_id,
        kind: InstallJobKind::from_stored_str(&row.kind),
        status: InstallJobStatus::from_stored_str(&row.status),
        state: serde_json::from_str(&row.state)?,
        created: timestamp(row.created),
        modified: timestamp(row.modified),
        finished: row.finished.and_then(optional_timestamp),
        dismissed: row.dismissed,
    })
}

fn instance_id(state: &InstallJobState) -> Option<String> {
    match &state.target {
        super::model::InstallTarget::NewInstance { instance_id } => {
            instance_id.clone()
        }
        super::model::InstallTarget::ExistingInstance { instance_id } => {
            Some(instance_id.clone())
        }
    }
}

fn timestamp(value: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

fn optional_timestamp(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(value, 0).single()
}
