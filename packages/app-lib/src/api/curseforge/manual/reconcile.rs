//! Manual download reconcile helpers.

use super::*;
use crate::State;
use crate::install::{InstallJobEventKind, InstallProgressReporter};
use crate::state::ContentProvider;
use std::collections::HashSet;

pub(crate) async fn reconcile_curseforge_waiting_jobs_for_instance(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    reconcile_curseforge_waiting_jobs_for_instance_with_state(
        instance_id,
        &state,
    )
    .await
}

pub(crate) async fn reconcile_persisted_curseforge_waiting_jobs(
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_persisted_curseforge_waiting_jobs_with_resume(
        state,
        &mut resume_job,
    )
    .await
}

pub(crate) async fn reconcile_persisted_curseforge_waiting_jobs_with_resume<
    F,
    Fut,
>(
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let jobs = crate::install::store::list(false, state).await?;
    for instance_id in curseforge_waiting_job_instance_ids(&jobs) {
        reconcile_curseforge_waiting_jobs_for_instance_with_resume(
            &instance_id,
            state,
            resume_job,
        )
        .await?;
    }
    Ok(())
}

pub(crate) fn curseforge_waiting_job_instance_ids(
    jobs: &[crate::install::store::InstallJobRecord],
) -> Vec<String> {
    let mut instance_ids = jobs
        .iter()
        .filter(|job| is_reconcilable_curseforge_waiting_job(job))
        .filter_map(|job| job.instance_id.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    instance_ids.sort_unstable();
    instance_ids
}

pub(crate) async fn reconcile_curseforge_waiting_jobs_for_instance_with_state(
    instance_id: &str,
    state: &State,
) -> crate::Result<()> {
    let mut resume_job = resume_curseforge_install_job;
    reconcile_curseforge_waiting_jobs_for_instance_with_resume(
        instance_id,
        state,
        &mut resume_job,
    )
    .await
}

pub(super) async fn resume_curseforge_install_job(
    job_id: uuid::Uuid,
) -> crate::Result<()> {
    crate::install::runner::resume_job(job_id).await?;
    Ok(())
}

pub(super) async fn reconcile_curseforge_waiting_jobs_for_instance_with_resume<
    F,
    Fut,
>(
    instance_id: &str,
    state: &State,
    resume_job: &mut F,
) -> crate::Result<()>
where
    F: FnMut(uuid::Uuid) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    let pending = crate::state::instances::adapters::sqlite::content_rows::get_pending_manual_downloads(
        instance_id,
        &state.pool,
    )
    .await?
    .into_iter()
    .filter(|download| download.provider == ContentProvider::CurseForge)
    .map(|download| {
        (download.provider_project_id, download.provider_release_id)
    })
    .collect::<HashSet<_>>();
    let materialized = crate::state::instances::adapters::sqlite::content_rows::get_materialized_curseforge_downloads(
        instance_id,
        &state.pool,
    )
    .await?
    .into_iter()
    .collect::<HashSet<_>>();

    let jobs = crate::install::store::list(false, state).await?;
    let waiting_job_count = jobs
        .iter()
        .filter(|job| {
            job.instance_id.as_deref() == Some(instance_id)
                && is_reconcilable_curseforge_waiting_job(job)
        })
        .count();
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        "Starting CurseForge waiting install job reconciliation"
    );
    let mut recovered_item_count = 0usize;
    let mut resume_job_called = false;
    for job in jobs {
        if job.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&job)
        {
            continue;
        }

        let operation_lock = state
            .install_job_operation_locks
            .entry(job.id)
            .or_default()
            .clone();
        let _operation_guard = operation_lock.lock().await;
        let current =
            crate::install::store::get_required(job.id, state).await?;
        if current.instance_id.as_deref() != Some(instance_id)
            || !is_reconcilable_curseforge_waiting_job(&current)
        {
            continue;
        }

        let mut reconciled_materialized = materialized.clone();
        reconciled_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &current.state,
            )
            .await?,
        );
        let reconciliation = curseforge_manual_download_reconciliation(
            &current.state.download_items(),
            &pending,
            &reconciled_materialized,
        );
        for inconsistent in &reconciliation.inconsistent {
            tracing::warn!(
                job_id = %current.id,
                instance_id,
                project_id = %inconsistent.project_id,
                file_id = %inconsistent.file_id,
                item_path = %inconsistent.path,
                reason = inconsistent.reason,
                "CurseForge manual download job state is inconsistent"
            );
        }

        let recovered_count = reconciliation.recovered.len();
        recovered_item_count += recovered_count;
        if recovered_count > 0 {
            InstallProgressReporter::new(current.id, current.state)
                .record_events(reconciliation.recovered)
                .await?;
        }

        let latest = crate::install::store::get_required(job.id, state).await?;
        if latest.status != crate::install::InstallJobStatus::WaitingForUser {
            continue;
        }
        let mut latest_materialized = materialized.clone();
        latest_materialized.extend(
            manually_imported_curseforge_world_downloads(
                instance_id,
                &latest.state,
            )
            .await?,
        );
        let latest_reconciliation = curseforge_manual_download_reconciliation(
            &latest.state.download_items(),
            &pending,
            &latest_materialized,
        );
        tracing::debug!(
            job_id = %latest.id,
            instance_id,
            pending_count = pending.len(),
            manual_skipped_count = reconciliation.manual_skipped_count,
            materialized_exact_match_count =
                reconciliation.materialized_exact_match_count,
            recovered_count,
            unresolved_pending_count =
                latest_reconciliation.unresolved_pending_count,
            inconsistent_count = latest_reconciliation.inconsistent.len(),
            "Reconciled CurseForge waiting install job"
        );

        if latest_reconciliation.should_resume() {
            resume_job_called = true;
            match resume_job(latest.id).await {
                Ok(_) => {
                    tracing::info!(
                        job_id = %latest.id,
                        instance_id,
                        pending_count = pending.len(),
                        manual_skipped_count =
                            reconciliation.manual_skipped_count,
                        materialized_exact_match_count =
                            reconciliation.materialized_exact_match_count,
                        recovered_count,
                        unresolved_pending_count = 0,
                        inconsistent_count = 0,
                        "Reconciled completed CurseForge manual downloads and resuming install job"
                    );
                }
                Err(error) => {
                    let current =
                        crate::install::store::get_required(latest.id, state)
                            .await?;
                    if current.status
                        == crate::install::InstallJobStatus::WaitingForUser
                    {
                        return Err(error);
                    }
                }
            }
        }
    }
    tracing::debug!(
        instance_id,
        pending_key_count = pending.len(),
        waiting_job_count,
        recovered_item_count,
        resume_job_called,
        "Completed CurseForge waiting install job reconciliation"
    );
    Ok(())
}

#[derive(Debug)]
pub(crate) struct CurseForgeManualDownloadInconsistency {
    pub(crate) path: String,
    pub(crate) project_id: String,
    pub(crate) file_id: String,
    pub(crate) reason: &'static str,
}

#[derive(Debug, Default)]
pub(crate) struct CurseForgeManualDownloadReconciliation {
    pub(crate) recovered: Vec<InstallJobEventKind>,
    pub(crate) inconsistent: Vec<CurseForgeManualDownloadInconsistency>,
    pub(crate) manual_skipped_count: usize,
    pub(crate) materialized_exact_match_count: usize,
    pub(crate) unresolved_pending_count: usize,
}

impl CurseForgeManualDownloadReconciliation {
    pub(crate) fn should_resume(&self) -> bool {
        self.unresolved_pending_count == 0 && self.inconsistent.is_empty()
    }
}

pub(crate) fn curseforge_manual_download_reconciliation(
    items: &[crate::install::model::DownloadItemSnapshot],
    pending: &HashSet<(String, String)>,
    materialized: &HashSet<(String, String)>,
) -> CurseForgeManualDownloadReconciliation {
    let mut result = CurseForgeManualDownloadReconciliation::default();
    for item in items.iter().filter(|item| {
        item.status == crate::install::model::DownloadItemStatus::Skipped
            && item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    }) {
        result.manual_skipped_count += 1;
        let project_id = item.project_id.as_ref().expect("filtered project ID");
        let version_id = item.version_id.as_ref().expect("filtered file ID");
        if pending.contains(&(project_id.clone(), version_id.clone())) {
            result.unresolved_pending_count += 1;
        } else if materialized
            .contains(&(project_id.clone(), version_id.clone()))
        {
            result.materialized_exact_match_count += 1;
            result
                .recovered
                .push(InstallJobEventKind::ContentFileRecovered {
                    path: item.id.clone(),
                    bytes: item.bytes_total.unwrap_or(0),
                });
        } else {
            result
                .inconsistent
                .push(CurseForgeManualDownloadInconsistency {
                    path: item.id.clone(),
                    project_id: project_id.clone(),
                    file_id: version_id.clone(),
                    reason: "pending_missing_but_not_materialized",
                });
        }
    }
    result
}

pub(super) fn is_reconcilable_curseforge_waiting_job(
    job: &crate::install::store::InstallJobRecord,
) -> bool {
    job.status == crate::install::InstallJobStatus::WaitingForUser
        && job.instance_id.is_some()
        && is_curseforge_manual_download_job(&job.state)
        && matches!(
            &job.state.pause_reason,
            Some(crate::install::model::InstallPauseReason::MissingRequiredContent { .. })
        )
        && job.state.download_items().iter().any(|item| {
            item.status == crate::install::model::DownloadItemStatus::Skipped
                && item.manual_url.is_some()
                && item.project_id.is_some()
                && item.version_id.is_some()
        })
}

pub(super) fn is_curseforge_manual_download_job(
    job: &crate::install::model::InstallJobState,
) -> bool {
    if !matches!(
        job.provider(),
        crate::install::model::InstallJobProvider::CurseForge
            | crate::install::model::InstallJobProvider::Local
    ) {
        return false;
    }
    job.download_items().iter().any(|item| {
        item.manual_url.is_some()
            && item.project_id.is_some()
            && item.version_id.is_some()
    })
}
