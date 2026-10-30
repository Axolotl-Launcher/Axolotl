use crate::install::InstallProgressReporter;
use tokio_util::sync::CancellationToken;

/// Owns a Minecraft core installation that runs alongside pack content work.
/// Dropping the guard cancels the task so an importer cannot leave a core
/// download running after its content installation fails.
pub(crate) struct ParallelMinecraftInstall {
    cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<crate::Result<()>>>,
    reporter: InstallProgressReporter,
}

impl ParallelMinecraftInstall {
    pub(crate) fn start(
        instance_id: String,
        reporter: InstallProgressReporter,
    ) -> Self {
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let parallel_reporter = reporter.with_parallel_output();
        let task_reporter = parallel_reporter.clone();
        let task = tokio::spawn(async move {
            tokio::select! {
                _ = task_cancel.cancelled() => {
                    tracing::debug!(
                        instance_id = %instance_id,
                        "Parallel Minecraft install aborted before completion"
                    );
                    Ok(())
                }
                result = crate::launcher::install_minecraft_for_instance_id_with_reporter(
                    &instance_id,
                    false,
                    Some(task_reporter),
                    crate::launcher::InstanceCompletionPolicy::DeferToInstallJob,
                ) => result,
            }
        });

        Self {
            cancel,
            task: Some(task),
            reporter: parallel_reporter,
        }
    }

    /// Cancels the core install and waits until the task has stopped.
    pub(crate) async fn abort(mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }

    /// Waits for the core install to finish without cancelling it.
    pub(crate) async fn join(mut self) -> crate::Result<()> {
        if self.task.as_ref().is_some_and(|task| !task.is_finished())
            && let Err(error) = self.reporter.foreground_parallel_output().await
        {
            tracing::warn!(%error, "Failed to publish parallel Minecraft progress");
        }
        if let Some(task) = self.task.take() {
            let result = task
                .await
                .map_err(crate::Error::from)
                .and_then(|result| result);
            if let Err(error) = result {
                if let Err(context_error) =
                    self.reporter.foreground_parallel_failure().await
                {
                    tracing::warn!(%context_error, "Failed to publish parallel Minecraft failure context");
                }
                return Err(error);
            }
        }
        Ok(())
    }
}

impl Drop for ParallelMinecraftInstall {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::{
        InstallErrorContext, InstallJobState, InstallPhaseDetails,
        InstallPhaseId, InstallRequest,
    };

    fn reporter() -> InstallProgressReporter {
        InstallProgressReporter::new(
            uuid::Uuid::new_v4(),
            InstallJobState::new(InstallRequest::DownloadJava {
                vendor: "test".into(),
                version: 21,
            }),
        )
    }

    #[tokio::test]
    async fn completed_core_failure_keeps_its_context_after_successful_local_extraction()
     {
        let main = reporter();
        let parallel = main.clone().with_parallel_output();
        parallel
            .update(
                InstallPhaseId::ResolvingMinecraft,
                None,
                InstallPhaseDetails::Empty,
            )
            .await
            .unwrap();
        let failed = InstallErrorContext::new("download Minecraft asset")
            .file_path("core-object")
            .build();
        parallel.persist_failure_context(failed).await;
        parallel
            .set_transient_context(
                InstallErrorContext::new("another core file").build(),
            )
            .await
            .unwrap();
        let task = tokio::spawn(async {
            Err(
                crate::ErrorKind::NetworkError("core transport failed".into())
                    .into(),
            )
        });
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
        main.set_transient_context(
            InstallErrorContext::new("extract modpack override")
                .file_path("overrides/options.txt")
                .build(),
        )
        .await
        .unwrap();
        let result = ParallelMinecraftInstall {
            cancel: CancellationToken::new(),
            task: Some(task),
            reporter: parallel,
        }
        .join()
        .await;
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("core transport failed")
        );
        let state = main.in_memory_state().await;
        assert_eq!(state.progress.phase, InstallPhaseId::ResolvingMinecraft);
        assert_eq!(
            state.context.unwrap().file_path.as_deref(),
            Some("core-object")
        );
    }

    #[tokio::test]
    async fn completed_success_does_not_replace_foreground_context_and_abort_joins_work()
     {
        let main = reporter();
        main.set_transient_context(
            InstallErrorContext::new("foreground")
                .file_path("override")
                .build(),
        )
        .await
        .unwrap();
        let task = tokio::spawn(async { Ok(()) });
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
        ParallelMinecraftInstall {
            cancel: CancellationToken::new(),
            task: Some(task),
            reporter: main.clone().with_parallel_output(),
        }
        .join()
        .await
        .unwrap();
        assert_eq!(
            main.in_memory_state()
                .await
                .context
                .unwrap()
                .file_path
                .as_deref(),
            Some("override")
        );
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let task = tokio::spawn(async move {
            token.cancelled().await;
            Ok(())
        });
        ParallelMinecraftInstall {
            cancel,
            task: Some(task),
            reporter: main,
        }
        .abort()
        .await;
    }
}
