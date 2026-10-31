use super::{Access, Entry, entry, text};
use serde_json::json;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let ids =
        json!({"type":"array","minItems":1,"maxItems":100,"items":text(256)});
    vec![
        entry(
            "resource_check_updates",
            "Check compatible updates for 1–100 exact entry_ids from content_list. Returns operation_id, then bounded update candidates/unresolved/failed IDs. Does not apply updates; dependency conflicts are checked by resource_prepare_update. Raw provider errors are omitted.",
            Controlled,
            true,
            true,
            json!({"instance_id":text(256),"entry_ids":ids}),
            &["instance_id", "entry_ids"],
        ),
        entry(
            "resource_prepare_update",
            "Review updating 1–100 installed content entries to automatic compatible releases with dependency analysis. Returns operation_id then a session-bound plan_id/revision and all actions/issues. Does not apply changes. Use resource_execute_changes (full access) after review; blocked plans can be revised with resource_revise_plan.",
            Controlled,
            false,
            false,
            json!({"instance_id":text(256),"entry_ids":ids}),
            &["instance_id", "entry_ids"],
        ),
        entry(
            "resource_prepare_switch",
            "Review replacing one installed content entry with an exact release of its existing source-qualified project. Source/project must match its known identity. Includes required dependencies. Returns operation_id then plan_id/revision. Does not apply changes; use resource_execute_changes (full access).",
            Controlled,
            false,
            false,
            json!({"instance_id":text(256),"entry_id":text(256),"source":{"type":"string","enum":["modrinth","curseforge","mcarchive","planet_minecraft"]},"project_id":text(256),"release_id":text(256)}),
            &[
                "instance_id",
                "entry_id",
                "source",
                "project_id",
                "release_id",
            ],
        ),
        entry(
            "resource_execute_changes",
            "Execute a reviewed resource update/version-switch plan in this same session using its exact revision. Requires full access because existing files may be replaced/deleted. Session handle expires in 10 minutes and is single-use. Rejects blocked/oversized/stale plans and active instances. Returns operation_id then installation job_id; poll install_job_get. Admitted jobs continue after session end.",
            FullAccess,
            false,
            false,
            json!({"plan_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1,"maximum":4294967295_u64}}),
            &["plan_id", "revision"],
        ),
    ]
}
