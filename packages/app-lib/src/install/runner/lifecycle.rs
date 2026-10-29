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

/// Extra runs a job gets when its runner fails before the job settles, such as
/// on a store write that timed out; the job stays queued between runs. The
/// same budget bounds retries of a settlement that fails.
const RUNNER_RETRIES: u32 = 3;
const RUNNER_RETRY_DELAY: std::time::Duration =
    std::time::Duration::from_secs(5);

pub(super) fn spawn_job(job_id: Uuid) {
    tokio::spawn(async move {
        for attempt in 0..=RUNNER_RETRIES {
            let Err(error) = EXECUTING_JOB.scope(job_id, run_job(job_id)).await
            else {
                return;
            };
            tracing::error!("Install job {job_id} failed: {error}");
            match settle_after_runner_error(job_id, attempt).await {
                Settled::RunAgain => {
                    tokio::time::sleep(RUNNER_RETRY_DELAY).await;
                }
                Settled::Done => return,
            }
        }
    });
}

#[derive(Debug, PartialEq, Eq)]
enum AfterRunnerError {
    /// Run the job again; it has not started.
    Retry,
    /// Roll the job back and mark it Interrupted; nothing drives it any more.
    Interrupt,
    /// Finish the cancellation the job was in.
    FinishCanceled,
    /// Nothing to do: the job is finished or waits for the user.
    Leave,
}

fn after_runner_error(
    status: Option<InstallJobStatus>,
    attempt: u32,
) -> AfterRunnerError {
    match status {
        // An unreadable status is as transient as the error that caused it.
        None | Some(InstallJobStatus::Queued) if attempt < RUNNER_RETRIES => {
            AfterRunnerError::Retry
        }
        // No job is left queued or running without a runner.
        Some(InstallJobStatus::Queued | InstallJobStatus::Running) => {
            AfterRunnerError::Interrupt
        }
        Some(InstallJobStatus::Canceling) => AfterRunnerError::FinishCanceled,
        _ => AfterRunnerError::Leave,
    }
}

enum Settled {
    RunAgain,
    Done,
}

/// Settles a job after `run_job` failed: it runs again, is interrupted or
/// finishes canceling. A settlement that fails is retried on the same budget;
/// only then is the job left as it is, with the failure logged.
async fn settle_after_runner_error(job_id: Uuid, attempt: u32) -> Settled {
    for settle_attempt in 0..=RUNNER_RETRIES {
        if settle_attempt > 0 {
            tokio::time::sleep(RUNNER_RETRY_DELAY).await;
        }
        match settle_once(job_id, attempt).await {
            Ok(settled) => return settled,
            Err(error) => {
                tracing::error!(%job_id, %error, settle_attempt, "Could not settle install job after its runner failed");
            }
        }
    }
    Settled::Done
}

async fn settle_once(job_id: Uuid, attempt: u32) -> crate::Result<Settled> {
    let state = State::get().await?;
    let record = match store::get(job_id, &state).await {
        Ok(Some(record)) => Some(record),
        Ok(None) => return Ok(Settled::Done),
        Err(error) => {
            tracing::warn!(%job_id, %error, "Could not read install job after its runner failed");
            None
        }
    };
    let decision =
        after_runner_error(record.as_ref().map(|job| job.status), attempt);
    let Some(record) = record else {
        return match decision {
            AfterRunnerError::Retry => Ok(Settled::RunAgain),
            _ => Err(crate::ErrorKind::OtherError(
                "the install job could not be read".into(),
            )
            .into()),
        };
    };
    match decision {
        AfterRunnerError::Retry => Ok(Settled::RunAgain),
        AfterRunnerError::Leave => Ok(Settled::Done),
        AfterRunnerError::Interrupt | AfterRunnerError::FinishCanceled => {
            // Settle under the job's own slot, so nothing else can start on
            // the instance while its files are rolled back. Without a slot
            // the job has finished, and there is nothing left to settle.
            let Some(_slot) =
                admission::settlement_slot(&state, &record).await?
            else {
                return Ok(Settled::Done);
            };
            // The job may have started canceling or finished while it
            // waited; settle what is stored now, not the record read before.
            let Some(record) = store::get(job_id, &state).await? else {
                return Ok(Settled::Done);
            };
            match after_runner_error(Some(record.status), RUNNER_RETRIES) {
                AfterRunnerError::Interrupt => {
                    recovery::settle_interrupted_job(
                        record,
                        recovery::Interruption::RunnerFailed,
                        &state,
                    )
                    .await?;
                }
                AfterRunnerError::FinishCanceled => {
                    let mut job_state = record.state;
                    finish_canceled_job(job_id, &mut job_state, &state).await?;
                }
                AfterRunnerError::Retry | AfterRunnerError::Leave => {}
            }
            Ok(Settled::Done)
        }
    }
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

    let Some((_execution, _install_permit)) =
        admission::queued_slot(&state, &job).await?
    else {
        return Ok(());
    };
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
    tracing::info!(%job_id, status = record.status.as_str(), phase = ?job_state.progress.phase, "Install job execution started");
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
            tracing::info!(%job_id, status = record.status.as_str(), phase = ?job_state.progress.phase, "Install job execution failed after rollback");
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

#[cfg(test)]
mod runner_error_tests {
    use super::*;

    #[test]
    fn a_job_that_never_started_runs_again_until_retries_run_out() {
        for status in [None, Some(InstallJobStatus::Queued)] {
            assert_eq!(after_runner_error(status, 0), AfterRunnerError::Retry);
            assert_eq!(
                after_runner_error(status, RUNNER_RETRIES - 1),
                AfterRunnerError::Retry
            );
        }
        assert_eq!(
            after_runner_error(Some(InstallJobStatus::Queued), RUNNER_RETRIES),
            AfterRunnerError::Interrupt,
            "a queued job is not left without a runner"
        );
    }

    #[test]
    fn a_running_job_without_a_runner_is_interrupted() {
        for attempt in [0, RUNNER_RETRIES] {
            assert_eq!(
                after_runner_error(Some(InstallJobStatus::Running), attempt),
                AfterRunnerError::Interrupt
            );
        }
    }

    #[test]
    fn a_canceling_job_finishes_its_cancellation() {
        assert_eq!(
            after_runner_error(Some(InstallJobStatus::Canceling), 0),
            AfterRunnerError::FinishCanceled
        );
    }

    #[test]
    fn settled_jobs_are_left_alone() {
        for status in [
            InstallJobStatus::Succeeded,
            InstallJobStatus::Failed,
            InstallJobStatus::Canceled,
            InstallJobStatus::Interrupted,
            InstallJobStatus::WaitingForUser,
        ] {
            assert_eq!(
                after_runner_error(Some(status), 0),
                AfterRunnerError::Leave
            );
        }
    }
}
