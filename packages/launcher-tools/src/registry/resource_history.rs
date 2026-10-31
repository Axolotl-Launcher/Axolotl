use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let source = json!({"type":"string","enum":["modrinth","curseforge","mcarchive","planet_minecraft"]});
    let project = json!({"source":source,"project_id":text(256)});
    let mut add = project.clone();
    add["resource_type"] = json!({"type":"string","enum":["mod","resource_pack","data_pack","shader"]});
    let mut list = page();
    list["instance_id"] = text(256);
    let mut history = list.clone();
    history["entry_id"] = text(256);
    vec![
        entry(
            "resource_favorites_list",
            "List saved resource favorites, newest first, paginated. Includes source-qualified project IDs and confirmed counterparts, content type and saved time. Does not fetch descriptions or install resources. A favorite may group confirmed projects across sources; no unconfirmed search candidate is treated as an equivalent.",
            Controlled,
            true,
            true,
            page(),
            &[],
        ),
        entry(
            "resource_favorite_add",
            "Save a source-qualified project as a favorite for mod/resource_pack/data_pack/shader. Validates current provider project/type before saving. Returns operation_id; poll operations_get. Repeated calls update the existing favorite rather than duplicating it. Does not install game content.",
            Controlled,
            false,
            true,
            add,
            &["source", "project_id", "resource_type"],
        ),
        entry(
            "resource_favorite_remove",
            "Remove a saved favorite and its confirmed source counterparts, matching the launcher's grouped-favorite behavior. Requires full access because favorite records are deleted. Does not uninstall resource files, remove provider projects or delete other unconfirmed candidates. Returns operation_id.",
            FullAccess,
            false,
            true,
            project,
            &["source", "project_id"],
        ),
        entry(
            "resource_history_list",
            "List per-instance resource operation history, optionally filtered by entry_id, newest first with pagination. Shows prior/target installed paths/releases and whether the verified local rollback backup remains available. Does not return acquisition URLs, dependency documents or internal backup file paths. backup_available is a snapshot, not a guarantee a future rollback passes runtime state/hash checks.",
            Controlled,
            true,
            true,
            history,
            &["instance_id"],
        ),
        entry(
            "resource_history_rollback",
            "Queue native rollback of an exact history_id/entry_id belonging to this instance, replacing current content with its verified local historical backup. Requires full access because current resource files are replaced/removed. Refuses a running game, active instance jobs/backups, stale target metadata or unavailable/changed backups. Returns operation_id, then install job_id; poll install_job_get. This is per-resource rollback, not a full instance restore; do not retry blindly.",
            FullAccess,
            false,
            false,
            json!({"instance_id":text(256),"entry_id":text(256),"history_id":text(256)}),
            &["instance_id", "entry_id", "history_id"],
        ),
        entry(
            "resource_scan_backups",
            "Scan native resource-operation backup files in one stopped launcher-owned instance. Returns operation_id; result is a paginated list with backup_id, relative path, size, provenance/exists flags and rollback availability. Can recover pending native cleanup journals. Does not initiate new backup deletion; refuses active games/jobs/backups and external layouts. Use returned backup IDs, never arbitrary paths, for cleanup preparation.",
            Controlled,
            false,
            true,
            list,
            &["instance_id"],
        ),
        entry(
            "resource_prepare_history_cleanup",
            "Prepare deleting selected distinct backup_ids returned by resource_scan_backups, maximum 100. Native validation checks ownership, provenance, file integrity, current-content references and retained-history dependencies. Returns operation_id, then a session-owned plan_id/revision and reviewed files/byte count. Preparing does not perform the selected deletions, but native recovery may finish/undo an older journal. Cleaning these files can permanently remove rollback capability.",
            Controlled,
            false,
            false,
            json!({"instance_id":text(256),"backup_ids":{"type":"array","minItems":1,"maxItems":100,"items":text(256)}}),
            &["instance_id", "backup_ids"],
        ),
        entry(
            "resource_execute_history_cleanup",
            "Execute a reviewed resource cleanup plan in this same control session using its exact revision. Requires full access. Handle expires after 10 minutes and is single-use. Rechecks instance revision, file hashes/provenance and remaining references through native rules; refuses active games/jobs/backups. Returns operation_id then removed_count/failure summaries; partial native failure may require recovery. Does not remove currently installed resource files or instance snapshot backups.",
            FullAccess,
            false,
            false,
            json!({"plan_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":0,"maximum":4294967295_u64}}),
            &["plan_id", "revision"],
        ),
    ]
}
