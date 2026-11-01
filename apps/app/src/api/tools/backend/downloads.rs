use super::{business_error, jobs, page};
use launcher_tools::{Command, ToolError};
use serde_json::{Value, json};
use theseus::install::{self, DownloadItemSnapshot, InstallJobSnapshot};

fn summary(job: InstallJobSnapshot) -> Value {
    let progress = &job.summary;
    let download = json!({"files_completed":progress.files_completed,"files_total":progress.files_total,"bytes_downloaded":progress.bytes_downloaded,
        "bytes_total":progress.bytes_total,"speed_bytes_per_second":progress.speed_bytes_per_second,"eta_seconds":progress.eta_seconds,"fallback_count":progress.fallback_count});
    let provider = job.provider;
    let item_count = job.items.len();
    let mut result = jobs::summary(job);
    result["downloads"] = download;
    result["provider"] = json!(provider);
    result["item_count"] = json!(item_count);
    result
}

fn item_summary(item: DownloadItemSnapshot) -> Value {
    json!({"item_id":item.id,"name":item.name.chars().take(256).collect::<String>(),"status":item.status,
        "bytes_downloaded":item.bytes_downloaded,"bytes_total":item.bytes_total,"attempt":item.attempt,"max_attempts":item.max_attempts,
        "has_error":item.error.is_some(),"manual_download_required":item.manual_url.is_some()})
}

fn dismissible(job: &InstallJobSnapshot) -> Result<(), ToolError> {
    if !job.status.is_finished() {
        return Err(ToolError::new(
            "job_active",
            "Cancel or finish the job before dismissing its history",
        ));
    }
    Ok(())
}

fn matches_filter(
    job: &InstallJobSnapshot,
    status: Option<&str>,
    provider: Option<&str>,
    query: Option<&str>,
) -> bool {
    status.is_none_or(|status| job.status.as_str() == status)
        && provider.is_none_or(|provider| job.provider.as_str() == provider)
        && query.is_none_or(|query| {
            let query = query.trim().to_lowercase();
            query.is_empty()
                || job.job_id.to_string().contains(&query)
                || job.display.as_ref().is_some_and(|display| {
                    display.title.to_lowercase().contains(&query)
                })
        })
}

pub(super) async fn execute(command: Command) -> Result<Value, ToolError> {
    match command {
        Command::DownloadJobList {
            status,
            provider,
            query,
            offset,
            limit,
        } => {
            let mut jobs =
                install::list_jobs(true).await.map_err(business_error)?;
            jobs.retain(|job| {
                matches_filter(
                    job,
                    status.as_deref(),
                    provider.as_deref(),
                    query.as_deref(),
                )
            });
            jobs.sort_by(|a, b| {
                b.created.cmp(&a.created).then(a.job_id.cmp(&b.job_id))
            });
            page(jobs.into_iter().map(summary).collect(), offset, limit)
        }
        Command::DownloadJobGet { job_id } => Ok(summary(
            install::get_job(job_id).await.map_err(business_error)?,
        )),
        Command::DownloadJobGetItems {
            job_id,
            offset,
            limit,
        } => {
            let job = install::get_job(job_id).await.map_err(business_error)?;
            let mut result = page(
                job.items.into_iter().map(item_summary).collect(),
                offset,
                limit,
            )?;
            result["job_id"] = json!(job_id);
            Ok(result)
        }
        Command::DownloadJobRetry { job_id } => Ok(summary(
            install::retry_job_as_new(job_id)
                .await
                .map_err(business_error)?,
        )),
        Command::DownloadJobResume { job_id } => Ok(summary(
            install::resume_job(job_id).await.map_err(business_error)?,
        )),
        Command::DownloadJobCancel { job_id } => Ok(summary(
            install::cancel_job(job_id).await.map_err(business_error)?,
        )),
        Command::DownloadJobDelete { job_id } => {
            dismissible(
                &install::get_job(job_id).await.map_err(business_error)?,
            )?;
            install::dismiss_job(job_id).await.map_err(business_error)?;
            Ok(json!({"job_id":job_id,"dismissed":true}))
        }
        Command::DownloadHistoryClear {} => Ok(
            json!({"dismissed_count":install::clear_job_history().await.map_err(business_error)?}),
        ),
        Command::InstallJobMissingFiles {
            job_id,
            offset,
            limit,
        } => {
            let content = install::list_missing_modpack_files(job_id)
                .await
                .map_err(business_error)?;
            let mut result=page(content.files.into_iter().map(|file|json!({"item_id":file.item_id,"path":file.path,"expected_size":file.expected_size,"status":file.status,
                "has_error":file.last_error.is_some(),"attempt":file.attempt,"max_attempts":file.max_attempts,"manual_download_available":!file.browser_urls.is_empty()})).collect(),offset,limit)?;
            result["remaining"] = json!(content.remaining);
            result["job_id"] = json!(job_id);
            Ok(result)
        }
        Command::InstallJobScanMissingFiles { job_id } => {
            let scan = install::scan_missing_modpack_files(job_id, None)
                .await
                .map_err(business_error)?;
            Ok(
                json!({"job":summary(scan.job),"directory_available":scan.download_directory.is_some(),"remaining":scan.content.remaining,
                "imported_count":scan.imported_item_ids.len(),"mismatched_count":scan.mismatched_item_ids.len(),"rejected_count":scan.rejected_item_ids.len(),
                "checked_candidates":scan.checked_candidates,"pending_candidates":scan.pending_candidates,"error_count":scan.errors.len()}),
            )
        }
        Command::InstallJobSkipMissing { job_id } => Ok(summary(
            install::skip_missing_content_and_resume_job(job_id)
                .await
                .map_err(business_error)?,
        )),
        Command::InstallJobRepairAndRetry { job_id } => Ok(summary(
            install::repair_cache_and_retry_job(job_id)
                .await
                .map_err(business_error)?,
        )),
        _ => Err(ToolError::new(
            "unsupported_command",
            "Not a download/history operation",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> InstallJobSnapshot {
        serde_json::from_value(json!({"job_id":"00000000-0000-4000-8000-000000000001","instance_id":"instance","source_instance_id":null,"instance_deleted":false,
            "kind":"download_java","status":"failed","execution_mode":"normal","provider":"java","target":{"type":"new_instance","instance_id":null},
            "phase":"preparing_java","progress":null,"details":{"type":"empty"},"parallel":null,"display":{"title":"测试 Java 下载","icon":"https://private/icon"},
            "error":null,"rollback_error":null,"pause_reason":null,"upgrade_result":null,"content_change":null,
            "created":"2026-01-01T00:00:00Z","modified":"2026-01-01T00:00:00Z","finished":null,
            "summary":{"files_completed":1,"files_total":2,"bytes_downloaded":50,"bytes_total":100,"speed_bytes_per_second":10,"eta_seconds":5,"source":"https://secret/source?token=hidden","fallback_count":0},"items":[]
        })).unwrap()
    }

    #[test]
    fn history_filters_match_exact_status_provider_unicode_title_and_job_id() {
        let job = job();
        assert!(matches_filter(
            &job,
            Some("failed"),
            Some("java"),
            Some("JAVA 下载")
        ));
        assert!(matches_filter(&job, None, None, Some("测试")));
        assert!(matches_filter(&job, None, None, Some("000000000001")));
        assert!(!matches_filter(&job, Some("running"), None, None));
        assert!(!matches_filter(&job, None, Some("local"), None));
        assert!(!matches_filter(&job, None, None, Some("missing")));
    }
    #[test]
    fn summaries_keep_download_progress_without_source_urls_or_icons() {
        let value = summary(job());
        assert_eq!(value["downloads"]["bytes_downloaded"], 50);
        assert_eq!(value["downloads"]["eta_seconds"], 5);
        assert_eq!(value["provider"], "java");
        assert!(!value.to_string().contains("https://"));
        assert!(!value.to_string().contains("token=hidden"));
    }
    #[test]
    fn active_jobs_cannot_be_dismissed_by_mcp() {
        let mut job = job();
        assert!(dismissible(&job).is_ok());
        for status in [
            install::InstallJobStatus::Queued,
            install::InstallJobStatus::Running,
            install::InstallJobStatus::WaitingForUser,
            install::InstallJobStatus::Canceling,
        ] {
            job.status = status;
            assert_eq!(dismissible(&job).unwrap_err().code, "job_active");
        }
    }
    #[test]
    fn item_output_is_bounded_and_does_not_include_urls_or_raw_errors() {
        let item:DownloadItemSnapshot=serde_json::from_value(json!({"id":"item","name":"字".repeat(500),"status":"failed","bytes_downloaded":1,"bytes_total":10,"attempt":2,"max_attempts":3,
            "error":"private error token","manual_url":"https://secret/manual","request_url":"https://secret/request","source":"https://secret/source"})).unwrap();
        let value = item_summary(item);
        assert_eq!(value["name"].as_str().unwrap().chars().count(), 256);
        assert_eq!(value["has_error"], true);
        assert_eq!(value["manual_download_required"], true);
        assert!(!value.to_string().contains("secret"));
        assert!(!value.to_string().contains("private"));
    }
}
