pub mod curseforge;
pub mod generic;
pub mod modrinth;

use crate::resources::*;

pub fn manifest(
    plan: &ResourceExportPlan,
) -> ResourceResult<(&'static str, serde_json::Value)> {
    Ok(match plan.format {
        ResourceExportFormat::Modrinth => {
            ("modrinth.index.json", modrinth::manifest(plan)?)
        }
        ResourceExportFormat::GenericZip => {
            ("modrinth.index.json", generic::manifest(plan)?)
        }
        ResourceExportFormat::CurseForge => {
            ("manifest.json", curseforge::manifest(plan)?)
        }
    })
}
