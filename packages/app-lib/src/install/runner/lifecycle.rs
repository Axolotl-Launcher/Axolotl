use super::*;

struct CancellationRegistration<'a> {
    registrations:
        &'a dashmap::DashMap<Uuid, tokio_util::sync::CancellationToken>,
    id: Uuid,
}
impl Drop for CancellationRegistration<'_> {
    fn drop(&mut self) {
        self.registrations.remove(&self.id);
    }
}

pub(super) fn spawn_job(job_id: Uuid) {
    tokio::spawn(async move {
        if let Err(error) = run_job(job_id).await {
            tracing::error!("Install job {job_id} failed: {error}");
        }
    });
}

pub(super) fn begin_failed_job_rollback(
    job_state: &mut InstallJobState,
    error: &crate::Error,
) {
    let failed_phase = job_state.progress.phase;
    let error_view =
        install_error_view(failed_phase, error, job_state.context.clone());
    job_state.record_event(InstallJobEventKind::Failed {
        phase: failed_phase,
        code: error_view.code.clone(),
        message: error_view.message.clone(),
    });
    job_state.error = Some(error_view);
    job_state.progress.phase = InstallPhaseId::RollingBack;
    job_state.progress.progress = None;
    job_state.progress.details = InstallPhaseDetails::Empty;
    job_state.progress.parallel = None;
    job_state.record_event(InstallJobEventKind::RollbackStarted {
        cleanup: job_state.cleanup.clone(),
    });
}

pub(super) fn latest_failure_phase(
    execution_state: &InstallJobState,
    reporter_state: &InstallJobState,
) -> InstallPhaseId {
    let latest_phase = |state: &InstallJobState| {
        state
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.kind {
                InstallJobEventKind::PhaseStarted { phase, .. } => {
                    Some((event.at, *phase))
                }
                _ => None,
            })
    };
    match (latest_phase(execution_state), latest_phase(reporter_state)) {
        (Some(execution), Some(reporter)) if reporter.0 > execution.0 => {
            reporter.1
        }
        (Some(execution), _) => execution.1,
        (None, Some(reporter)) => reporter.1,
        (None, None) => reporter_state.progress.phase,
    }
}

pub(super) fn begin_waiting_for_user(
    job_state: &mut InstallJobState,
    reason: InstallPauseReason,
) {
    job_state.pause_reason = Some(reason.clone());
    job_state.error = None;
    job_state.rollback_error = None;
    job_state.context = None;
    job_state.progress.parallel = None;
    job_state.record_event(InstallJobEventKind::WaitingForUser { reason });
}

async fn run_job(job_id: Uuid) -> crate::Result<()> {
    let _metadata_usage = crate::storage::usage::lease().await?;
    let state = State::get().await?;
    let mut job = store::get_required(job_id, &state).await?;

    if job.status != InstallJobStatus::Queued {
        return Ok(());
    }

    let _install_permit = state.install_job_semaphore.acquire().await?;
    job = store::get_required(job_id, &state).await?;

    if job.status != InstallJobStatus::Queued {
        return Ok(());
    }

    let mut job_state = job.state.clone();
    job_state.record_event(InstallJobEventKind::JobStarted);
    let Some(record) = store::update_status_if(
        job_id,
        InstallJobStatus::Queued,
        InstallJobStatus::Running,
        &job_state,
        &state,
    )
    .await?
    else {
        return Ok(());
    };
    let cancellation = tokio_util::sync::CancellationToken::new();
    state
        .install_job_cancellations
        .insert(job_id, cancellation.clone());
    let _registration = CancellationRegistration {
        registrations: &state.install_job_cancellations,
        id: job_id,
    };
    emit_install_job(&record.snapshot()).await?;
    if store::get_required(job_id, &state).await?.status
        == InstallJobStatus::Canceling
    {
        cancellation.cancel();
    }
    let live_reporter = InstallProgressReporter::new(job_id, job_state.clone());

    enum RunResult {
        Completed(crate::Result<InstallExecutionOutcome<Option<String>>>),
        Canceled,
    }

    let result =
        if matches!(job_state.request, InstallRequest::ExportResource { .. }) {
            let result =
                request::run_request(job_id, &mut job_state, &state).await;
            if result.is_err() && cancellation.is_cancelled() {
                RunResult::Canceled
            } else {
                RunResult::Completed(result)
            }
        } else if matches!(
            job_state.request,
            InstallRequest::ChangeContent { .. }
        ) {
            RunResult::Completed(
                request::run_request(job_id, &mut job_state, &state).await,
            )
        } else {
            match crate::install::critical_section::run_cancelable(
                &cancellation,
                request::run_request(job_id, &mut job_state, &state),
            )
            .await
            {
                Some(result) => RunResult::Completed(result),
                None => RunResult::Canceled,
            }
        };
    let committed = crate::resources::plans::frozen_request_completed(
        &job_state.request,
        &state.pool,
    )
    .await;
    let resource_committed = match committed {
        Ok(value) => value,
        Err(error) => {
            job_state.error = Some(InstallErrorView::from_error(
                "database_error",
                job_state.progress.phase,
                &error,
                None,
            ));
            let record = store::update_status(
                job_id,
                InstallJobStatus::Interrupted,
                &job_state,
                &state,
            )
            .await?;
            emit_install_job(&record.snapshot()).await?;
            return Err(error);
        }
    };
    let result = if resource_committed {
        RunResult::Completed(Ok(InstallExecutionOutcome::Completed(
            current_instance_id(&job_state),
        )))
    } else {
        result
    };
    state.install_job_cancellations.remove(&job_id);
    let execution_state = job_state;
    let reporter_state = live_reporter.current_state().await?;
    let failure_phase = latest_failure_phase(&execution_state, &reporter_state);
    job_state = reporter_state;

    match result {
        RunResult::Completed(Ok(InstallExecutionOutcome::Completed(
            instance_id,
        ))) => {
            if let Some(instance_id) = instance_id.as_ref() {
                set_instance_id(&mut job_state, instance_id.clone());
            }
            if cancellation.is_cancelled()
                && !resource_committed
                && !matches!(
                    job_state.request,
                    InstallRequest::ExportResource { .. }
                )
            {
                finish_canceled_job(job_id, &mut job_state, &state).await?;
                return Ok(());
            }
            job_state.record_event(InstallJobEventKind::JobSucceeded {
                instance_id: current_instance_id(&job_state),
            });
            job_state.progress.phase = if matches!(
                job_state.request,
                InstallRequest::UpgradeUnmanagedInstance { .. }
            ) {
                InstallPhaseId::Completed
            } else {
                InstallPhaseId::Finalizing
            };
            job_state.progress.progress = None;
            job_state.progress.details = InstallPhaseDetails::Empty;
            job_state.progress.parallel = None;
            job_state.error = None;
            job_state.rollback_error = None;
            job_state.pause_reason = None;
            if !matches!(
                job_state.request,
                InstallRequest::ChangeContent { .. }
            ) {
                job_state.continuation = None;
            }
            job_state.missing_content = None;
            job_state.skipped_missing_content_paths.clear();
            job_state.context = None;
            let mut completed_state = job_state.clone();
            completed_state.rollback = None;
            let Some(record) =
                store::complete_running_job(job_id, &completed_state, &state)
                    .await?
            else {
                if store::get_required(job_id, &state).await?.status
                    == InstallJobStatus::Canceling
                {
                    finish_canceled_job(job_id, &mut job_state, &state).await?;
                }
                return Ok(());
            };
            if let Some(instance_id) = instance_id.as_ref()
                && let Err(error) =
                    emit_instance(instance_id, InstancePayloadType::Edited)
                        .await
            {
                tracing::warn!(
                    job_id = %job_id,
                    instance_id,
                    error = %error,
                    "Install job succeeded, but its final instance event could not be emitted"
                );
            }
            if resource_committed
                && let Some(instance_id) = instance_id.as_ref()
                && let Err(error) =
                    crate::api::instance::emit_content_changed(instance_id)
                        .await
            {
                tracing::warn!(%error, instance_id, "Could not emit committed resource content change");
            }
            if let Err(error) =
                recovery::discard_content_rollback(&mut job_state, &state).await
            {
                tracing::warn!(
                    job_id = %job_id,
                    error = %error,
                    "Install job succeeded, but rollback staging could not be discarded"
                );
            }
            if let Err(error) = emit_install_job(&record.snapshot()).await {
                tracing::warn!(
                    job_id = %job_id,
                    error = %error,
                    "Install job succeeded, but its final event could not be emitted"
                );
            }
        }
        RunResult::Completed(Ok(InstallExecutionOutcome::WaitingForUser(
            reason,
        ))) => {
            let mut waiting_state = job_state.clone();
            begin_waiting_for_user(&mut waiting_state, reason);
            let Some(record) = store::update_status_if(
                job_id,
                InstallJobStatus::Running,
                InstallJobStatus::WaitingForUser,
                &waiting_state,
                &state,
            )
            .await?
            else {
                if store::get_required(job_id, &state).await?.status
                    == InstallJobStatus::Canceling
                {
                    finish_canceled_job(job_id, &mut job_state, &state).await?;
                }
                return Ok(());
            };
            emit_install_job(&record.snapshot()).await?;
        }
        RunResult::Canceled => {
            finish_canceled_job(job_id, &mut job_state, &state).await?;
        }
        RunResult::Completed(Err(error)) => {
            if cancellation.is_cancelled() {
                finish_canceled_job(job_id, &mut job_state, &state).await?;
                return Ok(());
            }
            job_state.progress.phase = failure_phase;
            begin_failed_job_rollback(&mut job_state, &error);
            let cleanup_succeeded = match recovery::apply_cleanup(
                &mut job_state,
                &state,
            )
            .await
            {
                Err(rollback_error) => {
                    tracing::error!(
                        "Error rolling back failed install job {job_id}: {rollback_error}"
                    );
                    job_state.rollback_error = Some(install_error_view(
                        InstallPhaseId::RollingBack,
                        &rollback_error,
                        None,
                    ));
                    job_state.record_event(
                        InstallJobEventKind::RollbackFailed {
                            message: rollback_error.to_string(),
                        },
                    );
                    false
                }
                Ok(()) => true,
            };
            recovery::finalize_rollback_state(
                &mut job_state,
                cleanup_succeeded,
            );
            if cleanup_succeeded {
                clear_deleted_new_instance_id(&mut job_state);
            }
            let record = store::update_status(
                job_id,
                InstallJobStatus::Failed,
                &job_state,
                &state,
            )
            .await?;
            emit_install_job(&record.snapshot()).await?;
            return Err(error);
        }
    }

    Ok(())
}

async fn finish_canceled_job(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<()> {
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
    job_state.record_event(InstallJobEventKind::RollbackStarted {
        cleanup: job_state.cleanup.clone(),
    });
    let cleanup_succeeded =
        match recovery::apply_cleanup(job_state, state).await {
            Err(rollback_error) => {
                job_state.rollback_error = Some(install_error_view(
                    InstallPhaseId::RollingBack,
                    &rollback_error,
                    None,
                ));
                job_state.record_event(InstallJobEventKind::RollbackFailed {
                    message: rollback_error.to_string(),
                });
                false
            }
            Ok(()) => true,
        };
    recovery::finalize_rollback_state(job_state, cleanup_succeeded);
    if cleanup_succeeded {
        clear_deleted_new_instance_id(job_state);
    }
    let record = store::update_status(
        job_id,
        InstallJobStatus::Canceled,
        job_state,
        state,
    )
    .await?;
    emit_install_job(&record.snapshot()).await
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    #[tokio::test]
    async fn task_abort_always_unregisters_the_cancel_token() {
        let registrations = std::sync::Arc::new(dashmap::DashMap::new());
        let id = Uuid::new_v4();
        let active = registrations.clone();
        let started = std::sync::Arc::new(tokio::sync::Notify::new());
        let signal = started.clone();
        let task = tokio::spawn(async move {
            active.insert(id, tokio_util::sync::CancellationToken::new());
            let _registration = CancellationRegistration {
                registrations: &active,
                id,
            };
            signal.notify_one();
            std::future::pending::<()>().await;
        });
        started.notified().await;
        assert!(registrations.contains_key(&id));
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(!registrations.contains_key(&id));
    }
}
