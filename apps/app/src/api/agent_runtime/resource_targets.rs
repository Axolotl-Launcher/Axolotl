use super::types::ToolRequest;
use launcher_tools::{ToolDefinition, ToolError};
use serde_json::json;
use theseus::state::InstanceInstallStage;

/// Internal task policy only. MCP keeps its general, instance-optional search.
pub(super) fn definition(entry: &ToolDefinition) -> ToolDefinition {
    let mut entry = entry.clone();
    if entry.name == "resource_search" {
        entry.description = format!(
            "Agent Window requirement: searching individual content requires an existing, fully installed target instance and its actual instance_id. For a new custom modpack, create the instance first, wait for its installation job, then confirm it with instance_get or instance_check_installation before searching mods/dependencies. Never invent an instance ID or use its display name as an ID. Only published modpack discovery (resource_type=mod_pack) may omit the target instance. {}",
            entry.description
        ).into();
        entry.input_schema["properties"]["instance_id"]["description"] = json!(
            "Required for individual content (mod, plugin, data_pack, resource_pack, shader, world). Use the exact ID from instance_list/get or completed instance creation, never a proposed name. The target must be fully installed. May be omitted only for published modpack discovery (mod_pack)."
        );
    }
    entry
}

pub(super) async fn validate(request: &ToolRequest) -> Result<(), ToolError> {
    if request.name != "resource_search" {
        return Ok(());
    }
    let published_pack = request.name == "resource_search"
        && request
            .arguments
            .get("resource_type")
            .is_some_and(|value| value == "mod_pack");
    let instance_id = request
        .arguments
        .get("instance_id")
        .and_then(|value| value.as_str());
    let Some(instance_id) = instance_id else {
        return if published_pack {
            Ok(())
        } else {
            Err(ToolError::new(
                "instance_required",
                "Choose an existing target with instance_list/get before searching individual content. For a NEW custom modpack, first create the instance with instance_create, wait for its installation job to finish, and use its returned instance_id. Do not search using a proposed name or guessed ID.",
            ))
        };
    };
    let instance = theseus::instance::get(instance_id).await.map_err(|_| ToolError::new(
        "instance_lookup_failed",
        "Could not inspect the target instance. Refresh instance_list/get before retrying; do not guess a replacement ID."
    ))?.ok_or_else(|| ToolError::new(
        "instance_not_found",
        "The supplied instance_id does not identify an existing instance. Resolve the real ID through instance_list/get. If the task requests a NEW custom modpack and its instance has not been created, create it first and wait for installation. Do not repeat this ID, search without a target, or create a duplicate of an existing instance."
    ))?;
    if !published_pack
        && instance.instance.install_stage != InstanceInstallStage::Installed
    {
        return Err(ToolError::new(
            "instance_not_ready",
            "The target instance exists but its base installation is not complete. Follow its existing install_job_get until completion, or inspect instance_check_installation and repair the existing job when needed, before searching content. Do not create another instance or guess another ID.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn content_search_requires_a_target_before_any_business_lookup() {
        let mut request = ToolRequest {
            call_id: "call".into(),
            name: "resource_search".into(),
            arguments: json!({"source":"modrinth","query":"Create","resource_type":"mod"}).as_object().unwrap().clone(),
        };
        let error = validate(&request).await.unwrap_err();
        assert_eq!(
            serde_json::to_value(error).unwrap()["code"],
            "instance_required"
        );
        request.arguments["resource_type"] = json!("mod_pack");
        assert!(validate(&request).await.is_ok());
    }

    #[test]
    fn internal_guidance_does_not_change_the_shared_mcp_contract() {
        let shared = launcher_tools::registry::get("resource_search").unwrap();
        let internal = definition(shared);
        assert!(internal.description.contains("create the instance first"));
        assert!(!shared.description.contains("Agent Window requirement"));
        assert_eq!(
            internal.input_schema["required"],
            shared.input_schema["required"]
        );
        assert!(
            internal.input_schema["properties"]["instance_id"]["description"]
                .as_str()
                .unwrap()
                .contains("Required for individual content")
        );
    }
}
