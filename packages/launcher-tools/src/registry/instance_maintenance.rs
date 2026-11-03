use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let instance = json!({"instance_id":text(256)});
    vec![
        entry(
            "instance_duplicate",
            "Queue a copy of an existing stopped instance through native duplication. Returns operation_id; poll operations_get for install job_id/new instance_id, then install_job_get. Native duplication copies game content and creates a new instance using source name/version/link, not every per-instance launch override. Direct-linked external instances cannot be duplicated. Existing task/backup activity is rejected. Repeated calls create additional copies; do not retry blindly.",
            Controlled,
            false,
            false,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "instance_reinstall",
            "Queue installation/repair of an existing stopped managed instance with its current game/loader configuration. Explicit force flag controls native forced reinstallation. Returns operation_id, then install job_id; poll install_job_get. Does not change game/loader versions or repair a managed modpack via a different pack version. Rejects external layouts, active jobs or backups. May replace managed game/loader files; back up first if necessary. Admitted installation jobs continue after control ends.",
            Controlled,
            false,
            false,
            json!({"instance_id":text(256),"force":{"type":"boolean"}}),
            &["instance_id", "force"],
        ),
        entry(
            "instance_check_installation",
            "Read recorded install stage and filesystem presence for one instance, including running processes, active install jobs and backup operations. This is metadata/status inspection, NOT a hash audit of every game file. installed=true reflects the runtime's recorded stage; a missing directory still makes ready_for_launch=false. External/direct-linked layouts are identified explicitly.",
            Controlled,
            true,
            true,
            instance.clone(),
            &["instance_id"],
        ),
        entry(
            "instance_delete_preview",
            "Preview permanent removal of a stopped ordinary instance stored directly inside the launcher's instances directory, including ALL its game files/worlds/ZIP backups and associated snapshot backups. Rejects linked/external/custom layouts and active jobs/backups. Returns session-bound preview_id with 10-minute expiry, target path and backup counts/sizes. Configuration, directory identity and snapshot list are rechecked before deletion; file contents are not frozen. Does not delete anything.",
            Controlled,
            true,
            true,
            instance,
            &["instance_id"],
        ),
        entry(
            "instance_delete",
            "Permanently remove the instance and ALL associated snapshots described by instance_delete_preview. Requires full access and the same control session. Supply only preview_id; arbitrary paths/IDs cannot replace the preview. Rechecks metadata, directory identity, snapshots and activity, rejecting stale previews. Single-use handle is consumed on an admitted deletion attempt; preview again after failure. Returns operation_id; poll operations_get. No automatic backup is created and native failure may leave partial cleanup; inspect state before retrying.",
            FullAccess,
            false,
            false,
            json!({"preview_id":{"type":"string","format":"uuid"}}),
            &["preview_id"],
        ),
    ]
}
