pub(crate) mod critical_section;
mod diagnostics;
pub mod events;
pub mod import_plan;
pub(crate) mod missing_content;
pub mod model;
pub mod recovery;
pub mod runner;
pub mod store;

pub use events::InstallProgressReporter;
pub use import_plan::{
    ImportPlanCounts, ImportPlanRequest, ImportPlanSnapshot, ImportPlanStage,
    cancel_import_plan, start_import_plan,
};
pub use missing_content::{
    MissingModpackContentView, MissingModpackFileView, MissingModpackScanError,
    MissingModpackScanResult, import_missing_modpack_file,
    list_missing_modpack_files, retry_missing_modpack_file,
    scan_missing_modpack_files,
};
pub use model::{
    CONTENT_CHANGE_PLAN_VERSION, ContentChangeAction,
    ContentChangeActionSnapshot, ContentChangeActionStatus,
    ContentChangeDependency, ContentChangeFile, ContentChangeFileIntegrity,
    ContentChangeFileRole, ContentChangeIntent, ContentChangeOperation,
    ContentChangeSnapshot, ContentChangeTarget, DownloadItemSnapshot,
    DownloadItemStatus, DownloadJobSummary, InstallContentBatchItem,
    InstallErrorContext, InstallErrorView, InstallJavaStep,
    InstallJobEventKind, InstallJobKind, InstallJobProvider,
    InstallJobSnapshot, InstallJobStatus, InstallModpackPreview,
    InstallPhaseDetails, InstallPhaseId, InstallPostInstallEdit,
    InstallProgress, InstallProgressSecondary, InstallRequest,
    InstanceUpgradeCompatibilityWarning, InstanceUpgradeDisplayNames,
    InstanceUpgradeExecution, InstanceUpgradeExternalChange,
    InstanceUpgradeExternalChangeKind, InstanceUpgradeResult,
    InstanceUpgradeWatchBaseline, SharedUpgradeMode,
};
pub use runner::{
    cancel_job, clear_job_history, create_instance,
    create_instance_with_adjuncts, create_modpack_instance, dismiss_job,
    download_java, duplicate_instance, get_job, import_instance,
    import_instance_with_path, import_instance_with_plan,
    install_content_batch, install_curseforge_world, install_existing_instance,
    install_pack_to_existing_instance, job_support_details, list_jobs,
    queue_content_change, repair_cache_and_retry_job, resume_job, retry_job,
    retry_job_as_new, skip_missing_content_and_resume_job,
    update_managed_curseforge_modpack, upgrade_unmanaged_instance,
};

/// Replaces credentials and IP addresses in text the user may share publicly
/// (support reports, exported logs) with placeholders.
pub async fn censor_shared_text(
    text: String,
    state: &crate::State,
) -> crate::Result<String> {
    diagnostics::censor_support_text(text, &state.pool).await
}

/// Replaces credentials while preserving IP addresses for network diagnostics.
pub async fn censor_shared_credentials(
    text: String,
    state: &crate::State,
) -> crate::Result<String> {
    diagnostics::censor_credentials(text, &state.pool).await
}

/// Runs fallible work with bounded concurrency without dropping peer futures
/// after the first error. The cancellation token is tripped immediately so
/// producers stop adding work, while every already buffered task still gets
/// an opportunity to clean up staged files or restore materialized state.
pub(crate) async fn try_for_each_concurrent_draining<St, T, F, Fut>(
    stream: St,
    limit: Option<usize>,
    cancellation: tokio_util::sync::CancellationToken,
    mut task: F,
) -> crate::Result<()>
where
    St: futures::Stream<Item = T>,
    F: FnMut(T) -> Fut,
    Fut: std::future::Future<Output = crate::Result<()>>,
{
    use futures::StreamExt as _;

    let first_error =
        std::sync::Arc::new(tokio::sync::Mutex::new(None::<crate::Error>));
    let worker_error = first_error.clone();
    let worker_cancellation = cancellation.clone();
    stream
        .for_each_concurrent(limit, move |item| {
            let future = task(item);
            let first_error = worker_error.clone();
            let cancellation = worker_cancellation.clone();
            async move {
                if let Err(error) = future.await {
                    cancellation.cancel();
                    let mut first_error = first_error.lock().await;
                    if first_error.is_none() {
                        *first_error = Some(error);
                    }
                }
            }
        })
        .await;

    match first_error.lock().await.take() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn concurrent_failure_cancels_but_drains_peer_cleanup() {
        let cancellation = tokio_util::sync::CancellationToken::new();
        let cleaned = Arc::new(AtomicUsize::new(0));
        let result = try_for_each_concurrent_draining(
            futures::stream::iter(0..4),
            Some(2),
            cancellation.clone(),
            {
                let cleaned = cleaned.clone();
                move |index| {
                    let cleaned = cleaned.clone();
                    async move {
                        if index == 0 {
                            return Err(crate::ErrorKind::OtherError(
                                "test pipeline failure".to_string(),
                            )
                            .into());
                        }
                        tokio::task::yield_now().await;
                        cleaned.fetch_add(1, Ordering::Relaxed);
                        Ok(())
                    }
                }
            },
        )
        .await;

        assert!(result.is_err());
        assert!(cancellation.is_cancelled());
        assert_eq!(cleaned.load(Ordering::Relaxed), 3);
    }
}
