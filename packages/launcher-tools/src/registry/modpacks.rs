use super::{Access, Entry, entry, text};
use serde_json::json;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    vec![
        entry(
            "modpack_prepare_install",
            "Review an exact Modrinth or CurseForge modpack release for a new instance (omit instance_id) or an existing instance. Verifies project/type/release through the provider. Existing CurseForge targets must already be managed by this same project. Returns operation_id then session-bound plan_id and declared target metadata; this is not a file-by-file diff. Does not install. Execute requires full access; replacement can overwrite/remove existing pack content. No caller URLs accepted.",
            Controlled,
            false,
            false,
            json!({"source":{"type":"string","enum":["modrinth","curseforge"]},"project_id":text(256),"release_id":text(256),"name":text(256),"instance_id":text(256)}),
            &["source", "project_id", "release_id", "name"],
        ),
        entry(
            "modpack_prepare_local",
            "Inspect an absolute local modpack archive and review installing it into a new or existing instance. Requires full access for external file access. Uses native archive format detection, bounds source to 4 GiB and fingerprints contents for execution. Directories/symlinks are rejected. Returns operation_id then plan_id and target summary. No extraction or installation occurs here; native preview may cache an icon. This is not a file-by-file diff.",
            FullAccess,
            false,
            false,
            json!({"path":text(4096),"name":text(256),"instance_id":text(256)}),
            &["path", "name"],
        ),
        entry(
            "modpack_execute_install",
            "Queue one reviewed modpack installation/update from this session. Requires full access; pack overrides and pruning may replace/delete content. Single-use plan expires after 10 minutes. Rechecks existing instance metadata/idle state and local archive hash. Returns operation_id then job_id; use install_job_get/missing-file tools. New-instance calls create another instance if repeated with a new plan; do not retry blindly. Admitted jobs continue after control ends.",
            FullAccess,
            false,
            false,
            json!({"plan_id":{"type":"string","format":"uuid"}}),
            &["plan_id"],
        ),
    ]
}
