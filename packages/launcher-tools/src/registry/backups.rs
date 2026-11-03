use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let instance = json!({"instance_id":text(256)});
    let snapshot = json!({"instance_id":text(256),"snapshot_id":text(256)});
    let operation = json!({"instance_id":text(256),"backup_operation_id":{"type":"string","format":"uuid"}});
    let preview = json!({"preview_id":{"type":"string","format":"uuid"}});
    let mut snapshots = page();
    snapshots["instance_id"] = text(256);
    let mut operations = snapshots.clone();
    operations["active_only"] = json!({"type":"boolean"});
    let paths = json!({"type":"array","maxItems":100,"items":{"type":"string","minLength":1,"maxLength":1024,"format":"relative-path"}});
    vec![
        entry(
            "backup_repository_status",
            "Read configured backup repository path, availability, size and snapshot counts. Does not move or initialize the repository.",
            Controlled,
            true,
            true,
            json!({}),
            &[],
        ),
        entry(
            "backup_get_config",
            "Read an instance's backup configuration, exclusions, snapshot count and eligibility. External-linked instances and unsupported layouts may be ineligible.",
            Controlled,
            true,
            true,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "backup_enable",
            "Enable backups for an eligible instance, preserving its current exclusions. Does not create a snapshot. Returns operation_id; poll operations_get for configuration.",
            Controlled,
            false,
            true,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "backup_disable",
            "Disable backups only when no snapshots remain, as required by the runtime. Does not delete snapshots; fails if any exist. Returns operation_id. Use backup_get_config to inspect configuration first.",
            Controlled,
            false,
            true,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "backup_set_exclusions",
            "Replace backup exclusions using instance-relative forward-slash paths: files and directories arrays are both required; empty arrays clear exclusions. Backups must already be enabled. Runtime validates paths, ancestor links and path types. Returns operation_id.",
            Controlled,
            false,
            true,
            json!({"instance_id":text(256),"files":paths,"directories":paths}),
            &["instance_id", "files", "directories"],
        ),
        entry(
            "backup_create",
            "Start a snapshot for an instance with backups enabled. Returns operation_id; its result contains backup_operation_id. Poll backup_get_operation until completed/failed/cancelled/interrupted. The native job continues after control ends. Retrying can create another snapshot.",
            Controlled,
            false,
            false,
            instance,
            &["instance_id"],
        ),
        entry(
            "backup_list_snapshots",
            "List an instance's backup snapshots newest first, with pagination, byte sizes and file counts. Use snapshot_id only with its owning instance.",
            Controlled,
            true,
            true,
            snapshots,
            &["instance_id"],
        ),
        entry(
            "backup_list_operations",
            "List persisted native backup jobs for one instance, optionally active_only, newest first. Paginated. Native restoration tokens and raw errors are omitted. These jobs are distinct from MCP operations_get records.",
            Controlled,
            true,
            true,
            operations,
            &["instance_id"],
        ),
        entry(
            "backup_get_operation",
            "Read one native backup job by backup_operation_id and instance_id. Reports progress/cancellability and terminal state. Completed creation returns snapshot_id. Restoration tokens are never exposed; use backup_get_restore_preview for a reviewed restore handle.",
            Controlled,
            true,
            true,
            operation.clone(),
            &["instance_id", "backup_operation_id"],
        ),
        entry(
            "backup_cancel",
            "Request cancellation of one native backup job belonging to the instance. Returns accepted=false when not cancellable or already past a safe cancellation point. Never forces cancellation during atomic restore application.",
            Controlled,
            false,
            true,
            operation,
            &["instance_id", "backup_operation_id"],
        ),
        entry(
            "backup_preview_restore",
            "Start computing restoration impact for one snapshot owned by the instance. Returns operation_id; poll operations_get for preview_id and backup_operation_id. Then poll backup_get_restore_preview. Does not restore files. Preview handle belongs to this session and expires after 10 minutes.",
            Controlled,
            false,
            false,
            snapshot.clone(),
            &["instance_id", "snapshot_id"],
        ),
        entry(
            "backup_get_restore_preview",
            "Poll a session-owned preview_id. When ready, returns counts of added/modified/deleted files. This same preview_id is the single-use input to backup_restore. Native restore token remains private. Repeated queries keep the same handle and expiry.",
            Controlled,
            true,
            true,
            preview.clone(),
            &["preview_id"],
        ),
        entry(
            "backup_restore",
            "Restore using a completed preview_id from this same control session. Requires full access because restoration replaces and may delete current files. Returns operation_id; its result contains backup_operation_id. Poll backup_get_operation for final result. Runtime rejects stale filesystem/snapshot fingerprints. A consumed handle cannot be retried; preview again after failure. Native job continues after control ends.",
            FullAccess,
            false,
            false,
            preview,
            &["preview_id"],
        ),
        entry(
            "backup_delete_snapshot",
            "Permanently delete one snapshot belonging to the specified instance through normal backup repository rules. Requires full access. Does not remove the instance. Returns operation_id; poll operations_get. Do not retry blindly.",
            FullAccess,
            false,
            false,
            snapshot,
            &["instance_id", "snapshot_id"],
        ),
    ]
}
