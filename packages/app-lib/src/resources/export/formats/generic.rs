use crate::resources::*;

pub fn manifest(
    plan: &ResourceExportPlan,
) -> ResourceResult<serde_json::Value> {
    super::modrinth::index(plan, false)
}
