use super::super::plan::invalid;
use crate::resources::{providers::positive_id, *};
use serde_json::{Value, json};

pub fn manifest(plan: &ResourceExportPlan) -> ResourceResult<Value> {
    let mut loaders = vec![];
    if let Some(loader) = &plan.requirements.loader {
        if !matches!(loader.as_str(), "forge" | "neoforge" | "fabric" | "quilt")
        {
            return Err(invalid(
                "this loader cannot be expressed by the CurseForge format",
            ));
        }
        let version = plan
            .requirements
            .loader_version
            .as_ref()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| invalid("loader version missing"))?;
        loaders.push(
            json!({"id": format!("{loader}-{version}"), "primary": true}),
        );
    }
    let mut files = vec![];
    for action in &plan.files {
        match action {
            ExportFileAction::Embed { .. } => {}
            ExportFileAction::Download { .. } => {
                return Err(invalid("download URL in a CurseForge export"));
            }
            ExportFileAction::CurseForgeReference { snapshot, artifact } => {
                if !crate::resources::identity::exact_integrity(
                    &snapshot.integrity,
                    &artifact.integrity,
                ) {
                    return Err(invalid(
                        "CurseForge artifact does not match the frozen bytes",
                    ));
                }
                if artifact.reference.release.project.source
                    != SourceId::curseforge()
                {
                    return Err(invalid(
                        "CurseForge export requires a CurseForge artifact",
                    ));
                }
                files.push(json!({"projectID": positive_id(artifact.reference.release.project.project_id.as_str())?, "fileID": positive_id(artifact.reference.release.release_id.as_str())?, "required": true}));
            }
        }
    }
    Ok(
        json!({"minecraft": {"version": plan.requirements.game_version, "modLoaders": loaders}, "manifestType": "minecraftModpack", "manifestVersion": 1, "name": plan.package_info.name, "version": plan.package_info.version, "author": plan.package_info.author.as_deref().unwrap_or(""), "summary": plan.package_info.summary, "files": files, "overrides": "overrides"}),
    )
}
