use super::*;
use crate::resources::{ResourceResult, export::plan::invalid};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub(super) async fn resolve(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    plan: &crate::resources::ResourceExportPlan,
    output: &Path,
    checkpoint: crate::resources::export::job::ExportResolutionCheckpoint,
) -> crate::Result<()> {
    validate(plan).await.map_err(ErrorKind::ResourceError)?;
    let state = State::get().await?;
    let root = crate::api::instance::get_full_path(&plan.instance_id).await?;
    let service = state.resources().await.map_err(ErrorKind::ResourceError)?;
    let reporter = InstallProgressReporter::new(job_id, job_state.clone());
    reporter
        .update(InstallPhaseId::Verifying, None, InstallPhaseDetails::Empty)
        .await?;
    let cancel = reporter.cancellation_token();
    let mut draft = checkpoint.restore();
    let known = crate::resources::export::refresh::known_references(
        &state.pool,
        &plan.instance_id,
    )
    .await
    .map_err(|e| ErrorKind::InputError(e.to_string()))?;
    let resolved = {
        let (updates, mut progress) = tokio::sync::watch::channel(
            None::<crate::resources::ExportRefreshProgress>,
        );
        let on_progress = move |value| {
            updates.send_replace(Some(value));
        };
        let resolving = crate::resources::export::job::resolve(
            service,
            &mut draft,
            plan,
            &root,
            &checkpoint.preferred_source,
            known,
            &cancel,
            &on_progress,
        );
        tokio::pin!(resolving);
        let mut tick =
            tokio::time::interval(std::time::Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut reported = None;
        loop {
            tokio::select! {
                result = &mut resolving => break result,
                _ = tick.tick() => {
                    let value = progress.borrow_and_update().clone();
                    if let Some(value) = value {
                        let current = ((value.overall_completed as f64 + value.current_file_fraction) * 1000.0) as u64;
                        let total = (value.overall_total as u64).saturating_mul(1000);
                        if reported != Some((current, total)) {
                            reporter.update(InstallPhaseId::Verifying, Some(InstallProgress { current, total, secondary: None }), InstallPhaseDetails::Empty).await?;
                            reported = Some((current, total));
                        }
                    }
                }
            }
        }
    };
    match resolved {
        Ok(resolved) => {
            reporter.checkpoint_export(resolved.clone(), None).await?;
            run(job_id, job_state, &resolved, output).await
        }
        Err(error) => {
            reporter.checkpoint_export(plan.clone(), Some(crate::resources::export::job::ExportResolutionCheckpoint::capture(&draft, checkpoint.preferred_source))).await?;
            Err(ErrorKind::ResourceError(error).into())
        }
    }
}

pub(super) async fn run(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    plan: &crate::resources::ResourceExportPlan,
    output: &Path,
) -> crate::Result<()> {
    validate(plan).await.map_err(ErrorKind::ResourceError)?;
    let root = crate::api::instance::get_full_path(&plan.instance_id).await?;
    let reporter = InstallProgressReporter::new(job_id, job_state.clone());
    let cancel = reporter.cancellation_token();
    let progress = Arc::new(AtomicU64::new(0));
    let total = crate::resources::export::writer::work_size(plan);
    let track = progress.clone();
    let on_progress = move |current, _| {
        track.store(current, Ordering::Relaxed);
    };
    let writing = crate::resources::export::writer::write_checked(
        &root,
        plan,
        output,
        &cancel,
        &on_progress,
        || async {
            validate(plan).await?;
            if crate::api::instance::get_full_path(&plan.instance_id).await?
                != root
            {
                return Err(invalid(
                    "game directory changed after export analysis",
                ));
            }
            Ok(())
        },
    );
    tokio::pin!(writing);
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reported = None;
    loop {
        tokio::select! {
            result = &mut writing => { result.map_err(ErrorKind::ResourceError)?; break; }
            _ = tick.tick() => {
                let current = progress.load(Ordering::Relaxed);
                if reported != Some(current) {
                    reporter.update(InstallPhaseId::Exporting, Some(InstallProgress { current, total, secondary: None }), InstallPhaseDetails::Empty).await?;
                    reported = Some(current);
                }
            }
        }
    }
    Ok(())
}

async fn validate(
    plan: &crate::resources::ResourceExportPlan,
) -> ResourceResult<()> {
    let state = State::get().await?;
    let current = crate::state::get_instance(&plan.instance_id, &state.pool)
        .await?
        .ok_or_else(|| invalid("instance no longer exists"))?;
    if current.applied_content_set.game_version
        != plan.requirements.game_version
        || current.applied_content_set.loader_version
            != plan.requirements.loader_version
        || (current.applied_content_set.loader != ModLoader::Vanilla)
            .then(|| current.applied_content_set.loader.as_str().to_string())
            != plan.requirements.loader
    {
        return Err(invalid(
            "game requirements changed after export analysis; reopen the export window",
        ));
    }
    if current.instance.is_direct_linked() {
        return Err(invalid(
            "directly associated instances cannot be exported",
        ));
    }
    Ok(())
}
