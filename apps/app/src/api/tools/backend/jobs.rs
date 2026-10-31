use super::{business_error, page};
use launcher_tools::ToolError;
use serde_json::{Value, json};
use theseus::install::InstallJobSnapshot;

pub(super) fn summary(job: InstallJobSnapshot) -> Value {
    json!({
        "job_id":job.job_id,
        "instance_id":job.instance_id,
        "instance_deleted":job.instance_deleted,
        "kind":job.kind,
        "status":job.status,
        "phase":job.phase,
        "progress":job.progress,
        "title":job.display.map(|display| display.title),
        "error_code":job.error.map(|error| error.code),
        "rollback_error_code":job.rollback_error.map(|error| error.code),
        "created":job.created,
        "modified":job.modified,
        "finished":job.finished,
    })
}

pub(super) async fn list(
    include_finished: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, ToolError> {
    let mut jobs = theseus::install::list_jobs(include_finished)
        .await
        .map_err(business_error)?;
    jobs.sort_by(|a, b| {
        b.created
            .cmp(&a.created)
            .then_with(|| a.job_id.cmp(&b.job_id))
    });
    page(jobs.into_iter().map(summary).collect(), offset, limit)
}

pub(super) async fn get(job_id: uuid::Uuid) -> Result<Value, ToolError> {
    Ok(summary(
        theseus::install::get_job(job_id)
            .await
            .map_err(business_error)?,
    ))
}
