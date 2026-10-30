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
        let result =
            worker_result(EXECUTING_JOB.scope(job_id, run_job(job_id))).await;
        if let Err(error) = result {
            tracing::error!(%job_id, %error, "Install worker recovery could not finish; persisted recovery information is retained");
        }
    });
}

async fn worker_result(
    work: impl Future<Output = crate::Result<()>>,
) -> crate::Result<()> {
    use futures::FutureExt;
    std::panic::AssertUnwindSafe(work)
        .catch_unwind()
        .await
        .unwrap_or_else(|_| {
            Err(ErrorKind::OtherError(
                "Install worker panicked; recovery is required".into(),
            )
            .into())
        })
}

pub(super) async fn recover_stopped_job(
    job_id: Uuid,
    state: &State,
    reporter: Option<&InstallProgressReporter>,
    error: &crate::Error,
) -> crate::Result<()> {
    let _usage = crate::storage::usage::lease().await?;
    let initial = store::get_required(job_id, state).await?;
    // Absence of a token can be a registration race. Ownership, followed by a
    // fresh status read, proves that no worker can still publish these files.
    let _execution = admission::execution_lock(state, &initial).await;
    recover_owned_job(job_id, state, reporter, error).await
}

async fn recover_owned_job(
    job_id: Uuid,
    state: &State,
    reporter: Option<&InstallProgressReporter>,
    error: &crate::Error,
) -> crate::Result<()> {
    let mut job = store::get_required(job_id, state).await?;
    if !needs_worker_recovery(job.status) {
        return Ok(());
    }
    if let Some(reporter) = reporter
        && let Ok(live) = reporter.current_state().await
    {
        if job.status == InstallJobStatus::Queued
            && live.events.iter().any(|event| {
                matches!(event.kind, InstallJobEventKind::JobStarted)
            })
        {
            // A failed generation may already have been explicitly retried.
            // Its retained reporter must not clean up that new queue entry.
            return Ok(());
        }
        job.state = live;
    }
    if complete_committed_job(job_id, &mut job.state, state).await? {
        return Ok(());
    }
    if job.status == InstallJobStatus::Canceling {
        finish_canceled_job(job_id, &mut job.state, state).await?;
        return Ok(());
    }
    begin_failed_job_rollback(&mut job.state, error);
    // Persist the removal of any resumable checkpoint before destructive
    // recovery starts, so a restart cannot treat a half rollback as a resume.
    if store::update_status_if(
        job_id, job.status, job.status, &job.state, state,
    )
    .await?
    .is_none()
    {
        if store::get_required(job_id, state).await?.status
            == InstallJobStatus::Canceling
        {
            finish_canceled_job(job_id, &mut job.state, state).await?;
        }
        return Ok(());
    }
    let cleaned = match recovery::apply_cleanup(&mut job.state, state).await {
        Ok(()) => true,
        Err(cleanup_error) => {
            job.state.rollback_error = Some(install_error_view(
                InstallPhaseId::RollingBack,
                &cleanup_error,
                None,
            ));
            job.state.record_event(InstallJobEventKind::RollbackFailed {
                message: cleanup_error.to_string(),
            });
            false
        }
    };
    recovery::finalize_rollback_state(&mut job.state, cleaned);
    if cleaned {
        clear_deleted_new_instance_id(&mut job.state);
    }
    loop {
        let expected = store::get_required(job_id, state).await?.status;
        if !needs_worker_recovery(expected) {
            return Ok(());
        }
        let status = if expected == InstallJobStatus::Canceling {
            job.state.error = Some(InstallErrorView::from_message(
                "canceled",
                job.state.progress.phase,
                "Install was canceled",
            ));
            InstallJobStatus::Canceled
        } else if cleaned {
            InstallJobStatus::Failed
        } else {
            InstallJobStatus::Interrupted
        };
        if let Some(record) =
            store::update_status_if(job_id, expected, status, &job.state, state)
                .await?
        {
            emit_job_best_effort(&record.snapshot()).await;
            return Ok(());
        }
    }
}

fn needs_worker_recovery(status: InstallJobStatus) -> bool {
    matches!(
        status,
        InstallJobStatus::Queued
            | InstallJobStatus::Running
            | InstallJobStatus::Canceling
    )
}

pub(super) async fn complete_committed_job(
    job_id: Uuid,
    job: &mut InstallJobState,
    state: &State,
) -> crate::Result<bool> {
    if !crate::resources::plans::frozen_request_completed(
        &job.request,
        &state.data,
    )
    .await?
    {
        return Ok(false);
    }
    job.error = None;
    job.rollback_error = None;
    job.pause_reason = None;
    job.context = None;
    job.progress.parallel = None;
    job.progress.progress = None;
    job.progress.details = InstallPhaseDetails::Empty;
    job.progress.phase = InstallPhaseId::Finalizing;
    job.continuation = None;
    job.missing_content = None;
    job.rollback = None;
    job.retry_from_checkpoint = false;
    job.record_event(InstallJobEventKind::JobSucceeded {
        instance_id: current_instance_id(job),
    });
    if let Some(record) =
        store::complete_running_job(job_id, job, state).await?
    {
        emit_job_best_effort(&record.snapshot()).await;
    }
    Ok(true)
}

pub(super) async fn emit_job_best_effort(snapshot: &InstallJobSnapshot) {
    if let Err(error) = emit_install_job(snapshot).await {
        tracing::warn!(job_id = %snapshot.job_id, %error, "Could not emit install task state");
    }
}

pub(super) fn begin_failed_job_rollback(
    job_state: &mut InstallJobState,
    error: &crate::Error,
) {
    job_state.retry_from_checkpoint = false;
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

pub(super) fn retain_pack_download(
    job: &mut InstallJobState,
    error: &crate::Error,
) -> bool {
    if matches!(error.raw.as_ref(), ErrorKind::FetchError(error) if error.is_decode())
    {
        return false;
    }
    let network_error = matches!(
        error.raw.as_ref(),
        ErrorKind::NetworkError(_)
            | ErrorKind::FetchError(_)
            | ErrorKind::HttpError { .. }
            | ErrorKind::DownloadH2Error(_)
            | ErrorKind::DownloadStreamStalled { .. }
            | ErrorKind::DownloadResourceWaitTimeout(_)
            | ErrorKind::DownloadTransferSlow
            | ErrorKind::DownloadEndpointChanged
            | ErrorKind::ApiIsDownError(_)
            | ErrorKind::ResourceError(
                crate::resources::ResourceError::Network { .. }
                    | crate::resources::ResourceError::Unavailable
            )
    );
    if !network_error || !job.is_new_pack_checkpoint() {
        return false;
    }
    let view =
        install_error_view(job.progress.phase, error, job.context.clone());
    job.record_event(InstallJobEventKind::Failed {
        phase: job.progress.phase,
        code: view.code.clone(),
        message: view.message.clone(),
    });
    job.error = Some(view);
    job.retry_from_checkpoint = true;
    job.pause_reason = None;
    job.progress.parallel = None;
    job.active_downloads.clear();
    true
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
    let job = store::get_required(job_id, &state).await?;

    if job.status != InstallJobStatus::Queued {
        return Ok(());
    }

    let (_execution, _install_permit) =
        match admission::execution_slot(&state, &job).await {
            Ok(slot) => slot,
            Err(error) => {
                drop(_metadata_usage);
                recover_stopped_job(job_id, &state, None, &error).await?;
                return Err(error);
            }
        };
    let reporter = InstallProgressReporter::new(job_id, job.state.clone());
    let result = worker_result(run_owned_job(job_id, &state)).await;
    if let Err(error) = &result {
        recover_owned_job(job_id, &state, Some(&reporter), error).await?;
    }
    result
}

async fn run_owned_job(job_id: Uuid, state: &State) -> crate::Result<()> {
    let job = store::get_required(job_id, state).await?;

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
        state,
    )
    .await?
    else {
        return Ok(());
    };
    tracing::info!(%job_id, status = record.status.as_str(), phase = ?job_state.progress.phase, "Install job execution started");
    let cancellation = tokio_util::sync::CancellationToken::new();
    state
        .install_job_cancellations
        .insert(job_id, cancellation.clone());
    let _registration = CancellationRegistration {
        registrations: &state.install_job_cancellations,
        id: job_id,
    };
    emit_job_best_effort(&record.snapshot()).await;
    if store::get_required(job_id, state).await?.status
        == InstallJobStatus::Canceling
    {
        cancellation.cancel();
    }
    let live_reporter = InstallProgressReporter::new(job_id, job_state.clone());

    enum RunResult {
        Completed(crate::Result<InstallExecutionOutcome<Option<String>>>),
        Canceled,
    }

    // The request future holds every kind of install, about 2 MB in debug
    // builds. Boxed here, the worker's enclosing futures stay small instead of
    // each carrying a copy of it on the stack.
    let result =
        if matches!(job_state.request, InstallRequest::ExportResource { .. }) {
            let result =
                Box::pin(request::run_request(job_id, &mut job_state, state))
                    .await;
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
                Box::pin(request::run_request(job_id, &mut job_state, state))
                    .await,
            )
        } else {
            match crate::install::critical_section::run_cancelable(
                &cancellation,
                Box::pin(request::run_request(job_id, &mut job_state, state)),
            )
            .await
            {
                Some(result) => RunResult::Completed(result),
                None => RunResult::Canceled,
            }
        };
    let committed = crate::resources::plans::frozen_request_completed(
        &job_state.request,
        &state.data,
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
                state,
            )
            .await?;
            emit_install_job(&record.snapshot()).await?;
            return Ok(());
        }
    };
    let result = if resource_committed {
        RunResult::Completed(Ok(InstallExecutionOutcome::Completed(
            current_instance_id(&job_state),
        )))
    } else {
        result
    };
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
                finish_canceled_job(job_id, &mut job_state, state).await?;
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
            job_state.retry_from_checkpoint = false;
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
                store::complete_running_job(job_id, &completed_state, state)
                    .await?
            else {
                if store::get_required(job_id, state).await?.status
                    == InstallJobStatus::Canceling
                {
                    finish_canceled_job(job_id, &mut job_state, state).await?;
                }
                return Ok(());
            };
            tracing::info!(%job_id, status = record.status.as_str(), phase = ?completed_state.progress.phase, "Install job execution completed");
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
                recovery::discard_content_rollback(&mut job_state, state).await
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
                state,
            )
            .await?
            else {
                if store::get_required(job_id, state).await?.status
                    == InstallJobStatus::Canceling
                {
                    finish_canceled_job(job_id, &mut job_state, state).await?;
                }
                return Ok(());
            };
            emit_install_job(&record.snapshot()).await?;
        }
        RunResult::Canceled => {
            finish_canceled_job(job_id, &mut job_state, state).await?;
        }
        RunResult::Completed(Err(error)) => {
            if cancellation.is_cancelled() {
                finish_canceled_job(job_id, &mut job_state, state).await?;
                return Ok(());
            }
            job_state.progress.phase = failure_phase;
            if retain_pack_download(&mut job_state, &error) {
                let Some(record) = store::update_status_if(
                    job_id,
                    InstallJobStatus::Running,
                    InstallJobStatus::Failed,
                    &job_state,
                    state,
                )
                .await?
                else {
                    if store::get_required(job_id, state).await?.status
                        == InstallJobStatus::Canceling
                    {
                        finish_canceled_job(job_id, &mut job_state, state)
                            .await?;
                    }
                    return Ok(());
                };
                tracing::info!(%job_id, instance_id = ?current_instance_id(&job_state), "Modpack download failed; keeping verified files for retry");
                emit_job_best_effort(&record.snapshot()).await;
                return Ok(());
            }
            begin_failed_job_rollback(&mut job_state, &error);
            if store::checkpoint_running_state(job_id, &job_state, state)
                .await?
                .is_none()
            {
                if store::get_required(job_id, state).await?.status
                    == InstallJobStatus::Canceling
                {
                    finish_canceled_job(job_id, &mut job_state, state).await?;
                }
                return Ok(());
            }
            let cleanup_succeeded = match recovery::apply_cleanup(
                &mut job_state,
                state,
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
            persist_failed_cleanup(job_id, &job_state, state).await?;
            tracing::info!(%job_id, phase = ?job_state.progress.phase, %error, "Install job execution failed after rollback");
            return Ok(());
        }
    }

    Ok(())
}

pub(super) async fn persist_failed_cleanup(
    job_id: Uuid,
    job: &InstallJobState,
    state: &State,
) -> crate::Result<()> {
    loop {
        let current = store::get_required(job_id, state).await?;
        let mut result = job.clone();
        let status = match current.status {
            InstallJobStatus::Running => InstallJobStatus::Failed,
            InstallJobStatus::Canceling => {
                result.error = Some(InstallErrorView::from_message(
                    "canceled",
                    job.progress.phase,
                    "Install was canceled",
                ));
                result.record_event(InstallJobEventKind::JobCanceled {
                    phase: job.progress.phase,
                });
                InstallJobStatus::Canceled
            }
            _ => return Ok(()),
        };
        if let Some(record) = store::update_status_if(
            job_id,
            current.status,
            status,
            &result,
            state,
        )
        .await?
        {
            emit_job_best_effort(&record.snapshot()).await;
            return Ok(());
        }
    }
}

async fn finish_canceled_job(
    job_id: Uuid,
    job_state: &mut InstallJobState,
    state: &State,
) -> crate::Result<()> {
    job_state.retry_from_checkpoint = false;
    let started = std::time::Instant::now();
    tracing::info!(%job_id, phase = ?job_state.progress.phase, "Install cancellation cleanup started after execution stopped");
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
    tracing::info!(%job_id, cleanup_succeeded, elapsed_ms = started.elapsed().as_millis() as u64, "Install cancellation cleanup completed");
    emit_install_job(&record.snapshot()).await
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    #[tokio::test]
    async fn worker_supervision_reports_errors_and_panics_without_reclassifying_success()
     {
        assert!(worker_result(async { Ok(()) }).await.is_ok());
        assert!(
            worker_result(async {
                Err(ErrorKind::NetworkError("source failed".into()).into())
            })
            .await
            .unwrap_err()
            .to_string()
            .contains("source failed")
        );
        assert!(
            worker_result(async { panic!("worker panic") })
                .await
                .unwrap_err()
                .to_string()
                .contains("worker panicked")
        );
        for status in [
            InstallJobStatus::Succeeded,
            InstallJobStatus::Failed,
            InstallJobStatus::Canceled,
            InstallJobStatus::Interrupted,
            InstallJobStatus::WaitingForUser,
        ] {
            assert!(!needs_worker_recovery(status));
        }
        for status in [
            InstallJobStatus::Running,
            InstallJobStatus::Queued,
            InstallJobStatus::Canceling,
        ] {
            assert!(needs_worker_recovery(status));
        }
    }
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
