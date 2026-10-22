use super::super::plan::invalid;
use crate::resources::*;
use serde_json::{Value, json};

pub fn manifest(plan: &ResourceExportPlan) -> ResourceResult<Value> {
    index(plan, true)
}

pub(super) fn index(
    plan: &ResourceExportPlan,
    standard: bool,
) -> ResourceResult<Value> {
    let mut dependencies = serde_json::Map::new();
    dependencies
        .insert("minecraft".into(), json!(plan.requirements.game_version));
    if let Some(loader) = &plan.requirements.loader {
        let key = match loader.as_str() {
            "forge" | "neoforge" => loader.as_str(),
            "cleanroom" if !standard => "cleanroom",
            "fabric" => "fabric-loader",
            "quilt" => "quilt-loader",
            _ => {
                return Err(invalid(
                    "this loader cannot be expressed by this export format",
                ));
            }
        };
        let version = plan
            .requirements
            .loader_version
            .as_ref()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| invalid("loader version missing"))?;
        dependencies.insert(key.into(), json!(version));
    }
    let mut files = vec![];
    for action in &plan.files {
        match action {
            ExportFileAction::Embed { .. } => {}
            ExportFileAction::CurseForgeReference { .. } => {
                return Err(invalid("CurseForge reference in an index export"));
            }
            ExportFileAction::Download {
                snapshot,
                artifact,
                environment,
            } => {
                if !crate::resources::identity::exact_integrity(
                    &snapshot.integrity,
                    &artifact.integrity,
                ) {
                    return Err(invalid(
                        "download artifact does not match the frozen bytes",
                    ));
                }
                if standard
                    && artifact.reference.release.project.source
                        != SourceId::modrinth()
                {
                    return Err(invalid(
                        "standard MR export requires a Modrinth artifact",
                    ));
                }
                let ArtifactAcquisition::PublicDownload { urls } =
                    &artifact.acquisition
                else {
                    return Err(invalid("export reference has no public URL"));
                };
                if urls.is_empty()
                    || urls.iter().any(|url| {
                        crate::resources::providers::public_url(url).is_none()
                    })
                {
                    return Err(invalid(
                        "export reference has an invalid public URL",
                    ));
                }
                let hash = |algorithm| {
                    snapshot
                        .integrity
                        .hashes
                        .iter()
                        .find(|h| h.algorithm == algorithm)
                        .map(|h| h.value.clone())
                        .ok_or_else(|| {
                            invalid("export reference has no current hash")
                        })
                };
                let mut file = json!({
                    "path": snapshot.relative_path,
                    "hashes": {"sha1": hash(HashAlgorithm::Sha1)?, "sha512": hash(HashAlgorithm::Sha512)?},
                    "downloads": urls, "fileSize": snapshot.integrity.size.ok_or_else(|| invalid("export size missing"))?,
                });
                let mut env = serde_json::Map::new();
                for (key, value) in [
                    ("client", environment.client),
                    ("server", environment.server),
                ] {
                    let value = match value {
                        ArtifactEnvironmentRequirement::Required => "required",
                        ArtifactEnvironmentRequirement::Optional => "optional",
                        ArtifactEnvironmentRequirement::Unsupported => {
                            "unsupported"
                        }
                        ArtifactEnvironmentRequirement::Unknown => continue,
                    };
                    env.insert(key.into(), json!(value));
                }
                if !env.is_empty() {
                    file["env"] = json!(env);
                }
                files.push(file);
            }
        }
    }
    let mut value = json!({"game": "minecraft", "formatVersion": 1, "versionId": plan.package_info.version, "name": plan.package_info.name, "summary": plan.package_info.summary, "files": files, "dependencies": dependencies});
    if let Some(author) = plan
        .package_info
        .author
        .as_ref()
        .filter(|a| !a.trim().is_empty())
    {
        value["author"] = json!(author);
    }
    Ok(value)
}
