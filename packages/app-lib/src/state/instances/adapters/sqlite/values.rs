use crate::state::ProjectType;
use chrono::{DateTime, TimeZone, Utc};

pub(super) fn project_type_from_str(value: &str) -> crate::Result<ProjectType> {
    match value {
        "mod" => Ok(ProjectType::Mod),
        "datapack" => Ok(ProjectType::DataPack),
        "resourcepack" => Ok(ProjectType::ResourcePack),
        "shader" | "shaderpack" => Ok(ProjectType::ShaderPack),
        "schematic" => Ok(ProjectType::Schematic),
        "world_save" => Ok(ProjectType::WorldSave),
        other => Err(crate::ErrorKind::InputError(format!(
            "Unknown content project type {other}"
        ))
        .into()),
    }
}

pub(super) fn timestamp(value: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(value, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

pub(super) fn optional_timestamp(value: i64) -> Option<DateTime<Utc>> {
    Utc.timestamp_opt(value, 0).single()
}

pub(super) fn unsigned(value: i64, column: &str) -> crate::Result<u64> {
    if value < 0 {
        return Err(crate::ErrorKind::InputError(format!(
            "Expected {column} to be non-negative"
        ))
        .into());
    }

    Ok(value as u64)
}
