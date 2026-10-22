//! Offline projection of compatible legacy payloads through the source adapters.

use serde_json::Value;

use super::*;
use crate::state::instances::adapters::sqlite::resource_rows::validate_project;

pub(crate) fn project(
    data_type: &str,
    data: Value,
) -> ResourceResult<ProjectDocument> {
    let document = match data_type {
        "project" => serde_json::from_value::<modrinth::RemoteProject>(data)?
            .document()?,
        "curseforge_project" => {
            let normalized = serde_json::to_value(serde_json::from_value::<
                crate::api::curseforge::CurseForgeProject,
            >(data)?)?;
            serde_json::from_value::<curseforge::RemoteProject>(normalized)?
                .document()?
        }
        _ => {
            return Err(ResourceError::Unsupported(
                "legacy project schema".into(),
            ));
        }
    };
    validate_project(&document.metadata.project)?;
    Ok(document)
}

pub(crate) fn release(data: Value) -> ResourceResult<ReleaseDocument> {
    let document =
        serde_json::from_value::<modrinth::RemoteRelease>(data)?.document()?;
    validate_project(&document.release.reference.project)?;
    let id = document.release.reference.release_id.as_str();
    if id.len() != 8 || !id.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err(ResourceError::InvalidResponse(
            "legacy release ID is not stable".into(),
        ));
    }
    Ok(document)
}

pub(crate) fn curseforge_file(data: Value) -> ResourceResult<ReleaseDocument> {
    serde_json::from_value::<curseforge::RemoteFile>(data)?.document()
}
