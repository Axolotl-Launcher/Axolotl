use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let job = json!({"job_id":{"type":"string","format":"uuid"}});
    let mut items = page();
    items["job_id"] = job["job_id"].clone();
    let mut list = page();
    list["status"] = json!({"type":"string","enum":["queued","running","canceling","waiting_for_user","succeeded","failed","interrupted","canceled"]});
    list["provider"] = json!({"type":"string","enum":["modrinth","curse_forge","minecraft","java","application","local"]});
    list["query"] = json!({"type":"string","maxLength":256});
    vec![
        entry(
            "download_job_list",
            "Search the launcher's unified download/install job history, newest first, paginated. Optional status/provider filters use exact enum values; query matches title or job UUID case-insensitively. Includes installation, Java and export jobs, not a separate download queue. Dismissed history is omitted. URLs and raw diagnostics are omitted.",
            Controlled,
            true,
            true,
            list,
            &[],
        ),
        entry(
            "download_job_get",
            "Read a unified download/install job with byte/file progress, speed/ETA and error codes. Does not expose request URLs, raw diagnostics or credentials. job_id is the same ID used by install_job_get.",
            Controlled,
            true,
            true,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "download_job_get_items",
            "Read a page of download items within a job: item ID, bounded name, state, byte progress and attempt counts. Omits request/manual URLs, source strings and raw errors. has_error/manual_download_required indicate further user action may be needed.",
            Controlled,
            true,
            true,
            items.clone(),
            &["job_id"],
        ),
        entry(
            "download_job_retry",
            "Retry a failed/interrupted/canceled unified job as a NEW job, preserving the original history. Returns operation_id; poll operations_get for the NEW job_id. Unlike install_job_retry this can repeat the entire original operation (including instance creation). Inspect kind/target first and do not retry blindly.",
            Controlled,
            false,
            false,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "download_job_resume",
            "Resume the same paused installation/download job using normal runtime rules. Returns operation_id; poll operations_get then install_job_get. Does not bypass required manual downloads.",
            Controlled,
            false,
            false,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "download_job_cancel",
            "Request normal cancellation/rollback for a unified job. Returns operation_id; poll operations_get and job status. Does not delete history. Runtime may roll back incomplete installation changes.",
            Controlled,
            false,
            false,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "download_job_delete",
            "Dismiss one finished job from visible history through native runtime rules. Requires full access. Returns operation_id. Does not delete downloaded game files, instances or the underlying job record. Active jobs cannot be dismissed.",
            FullAccess,
            false,
            true,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "download_history_clear",
            "Dismiss all finished jobs (succeeded/failed/interrupted/canceled) from unified download/install history. Requires full access. Returns operation_id; result has dismissed_count. Leaves active jobs, underlying records and downloaded/installed files intact.",
            FullAccess,
            false,
            true,
            json!({}),
            &[],
        ),
        entry(
            "install_job_missing_files",
            "List a page of required missing modpack files for a job waiting for user action, including item IDs, expected sizes and retry state. Omits browser URLs and raw errors. Returns remaining count; not all missing files are automatically downloadable.",
            Controlled,
            true,
            true,
            items,
            &["job_id"],
        ),
        entry(
            "install_job_scan_missing_files",
            "Scan only the OS Downloads directory for files matching this waiting job's missing items, then verify/import matching stable files using the runtime. Returns operation_id; poll operations_get for imported/rejected/mismatched counts and updated job. New files may need another scan after stabilizing. No arbitrary directory/path accepted.",
            Controlled,
            false,
            false,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "install_job_skip_missing",
            "Explicitly skip remaining required manual content and resume a waiting job, potentially producing an incomplete/broken instance. Returns operation_id. Runtime forbids this for managed CurseForge modpack version updates and unsupported jobs. Use only when skipping missing files is intended.",
            Controlled,
            false,
            false,
            job.clone(),
            &["job_id"],
        ),
        entry(
            "install_job_repair_and_retry",
            "Delete the runtime-selected metadata/resource caches associated with a failed job and retry the SAME job. Requires full access because cache entries are deleted. Returns operation_id. Uses the existing cache-repair eligibility/locking rules; no arbitrary cache paths or files accepted.",
            FullAccess,
            false,
            false,
            job,
            &["job_id"],
        ),
    ]
}
