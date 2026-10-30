use launcher_tools::{Access as SharedAccess, ToolDefinition, registry};
use rmcp::{
    ErrorData,
    model::{Tool, ToolAnnotations},
};
use serde_json::{Map, Value, json};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Access {
    Public,
    Session,
    FullAccess,
}

pub(crate) struct Entry {
    pub(crate) tool: Tool,
    pub(crate) access: Access,
    pub(crate) read_only: bool,
    pub(crate) deferred: bool,
}

fn shared_entry(definition: &ToolDefinition) -> Entry {
    let access = match definition.access {
        SharedAccess::Public => Access::Public,
        SharedAccess::Controlled => Access::Session,
        SharedAccess::FullAccess => Access::FullAccess,
    };
    let mut schema = definition.input_schema.clone();
    if access != Access::Public {
        schema["properties"].as_object_mut().unwrap().insert(
            "session_id".into(),
            json!({"type":"string","minLength":1,"maxLength":128}),
        );
        schema["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("session_id"));
    }
    Entry {
        tool: Tool::new(
            definition.name.clone(),
            definition.description.clone(),
            schema,
        )
        .with_annotations(
            ToolAnnotations::new()
                .read_only(definition.read_only)
                .destructive(definition.destructive)
                .idempotent(definition.idempotent)
                .open_world(definition.open_world),
        ),
        access,
        read_only: definition.read_only,
        deferred: definition.execution.deferred,
    }
}

fn lifecycle(
    name: &'static str,
    description: &'static str,
    access: Access,
) -> Entry {
    let (properties, required) = if access == Access::Public {
        (json!({}), json!([]))
    } else {
        (
            json!({"session_id":{"type":"string","minLength":1,"maxLength":128}}),
            json!(["session_id"]),
        )
    };
    let schema = json!({"type":"object","properties":properties,"required":required,"additionalProperties":false});
    Entry {
        tool: Tool::new(name, description, schema.as_object().unwrap().clone())
            .with_annotations(
                ToolAnnotations::new()
                    .read_only(false)
                    .destructive(false)
                    .idempotent(true)
                    .open_world(false),
            ),
        access,
        read_only: false,
        deferred: false,
    }
}

pub(crate) fn entries() -> Vec<Entry> {
    let mut entries: Vec<_> =
        registry::catalog().iter().map(shared_entry).collect();
    entries.insert(2, lifecycle("start_session", "Request launcher control for the user's task. May show approval; never call during startup or discovery. Returns session_id required by business tools. Call end_session when finished.", Access::Public));
    entries.insert(3, lifecycle("end_session", "Release control and remove the AI indicator. Keeps MCP transport available. Already admitted mutations finish; queued operations are cancelled. Installation jobs already created continue independently.", Access::Session));
    entries
}

pub(crate) fn definitions() -> Vec<Tool> {
    entries().into_iter().map(|entry| entry.tool).collect()
}

pub(crate) fn validate(
    entry: &Entry,
    arguments: &Map<String, Value>,
) -> Result<(), ErrorData> {
    registry::validate_schema(&entry.tool.input_schema, arguments)
        .map_err(|error| ErrorData::invalid_params(error.message, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_catalog_preserves_the_pre_refactor_contract() {
        let tools = definitions();
        assert_eq!(tools.len(), 269);
        let bytes = serde_json::to_vec(&tools).unwrap();
        // FNV-1a of the complete pre-refactor catalog: names, order, descriptions,
        // argument schemas and annotations. Update only for intentional API changes.
        let fingerprint =
            bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
            });
        assert_eq!(fingerprint, 0x3fe057d32cf1772d);
    }

    #[test]
    fn session_parameters_are_added_only_by_the_mcp_adapter() {
        for definition in registry::catalog() {
            let entry = shared_entry(definition);
            let mut schema = (*entry.tool.input_schema).clone();
            if definition.access != SharedAccess::Public {
                assert!(
                    schema["properties"]
                        .as_object_mut()
                        .unwrap()
                        .remove("session_id")
                        .is_some()
                );
                let required = schema["required"].as_array_mut().unwrap();
                assert_eq!(required.pop(), Some(json!("session_id")));
            }
            assert_eq!(schema, definition.input_schema);
            assert_eq!(entry.deferred, definition.execution.deferred);
        }
        assert!(registry::get("start_session").is_none());
        assert!(registry::get("end_session").is_none());
    }
}
