use super::{business_error, serialize};
use launcher_tools::ToolError;
use serde_json::Value;

pub(super) async fn processes(
    instance_id: Option<String>,
) -> Result<Value, ToolError> {
    let mut processes =
        theseus::process::get_all().await.map_err(business_error)?;
    if let Some(instance_id) = instance_id {
        processes.retain(|process| process.instance_id == instance_id);
    }
    processes.sort_by_key(|process| process.uuid);
    serialize(processes)
}

pub(super) async fn java() -> Result<Value, ToolError> {
    serialize(
        theseus::jre::get_registered_java_versions()
            .await
            .map_err(business_error)?,
    )
}

pub(super) async fn memory() -> Result<Value, ToolError> {
    serialize(
        theseus::jre::get_memory_status(None, 0, false)
            .await
            .map_err(business_error)?,
    )
}
