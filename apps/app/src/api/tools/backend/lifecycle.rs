use super::{business_error, serialize};
use launcher_tools::ToolError;
use serde_json::{Value, json};
use std::time::Duration;

pub(super) async fn launch(
    instance_id: String,
    offline_mode: bool,
) -> Result<Value, ToolError> {
    if !theseus::process::get_by_instance_id(&instance_id)
        .await
        .map_err(business_error)?
        .is_empty()
    {
        return Err(ToolError::new(
            "already_running",
            "This instance already has a running game process",
        ));
    }
    let process = theseus::instance::run(
        &instance_id,
        theseus::instance::QuickPlayType::None,
        offline_mode,
    )
    .await
    .map_err(business_error)?;
    Ok(json!({"process":process}))
}

pub(super) async fn get(process_id: uuid::Uuid) -> Result<Value, ToolError> {
    let process = theseus::process::get_all()
        .await
        .map_err(business_error)?
        .into_iter()
        .find(|process| process.uuid == process_id);
    Ok(json!({"running":process.is_some(),"process":process}))
}

pub(super) async fn wait(
    process_id: uuid::Uuid,
    timeout_seconds: u64,
) -> Result<Value, ToolError> {
    match tokio::time::timeout(
        Duration::from_secs(timeout_seconds.clamp(1, 20)),
        theseus::process::wait_for(process_id),
    )
    .await
    {
        Ok(result) => {
            result.map_err(business_error)?;
            Ok(json!({"exited":true}))
        }
        Err(_) => Ok(json!({"exited":false})),
    }
}

pub(super) async fn kill(process_id: uuid::Uuid) -> Result<Value, ToolError> {
    // Resolve against the launcher's tracked UUIDs; never interpret a PID from
    // client input or terminate a process outside the game process manager.
    let running = theseus::process::get_all()
        .await
        .map_err(business_error)?
        .iter()
        .any(|process| process.uuid == process_id);
    if !running {
        return Err(ToolError::new(
            "not_found",
            "Running game process not found",
        ));
    }
    theseus::process::kill(process_id)
        .await
        .map_err(business_error)?;
    serialize(json!({"stop_requested":true,"process_id":process_id}))
}
