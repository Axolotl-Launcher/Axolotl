use std::path::Path;

use daedalus::minecraft::{Argument, ArgumentType};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

pub(super) fn parse_version_json<T: DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
    include_patches: bool,
) -> crate::Result<T> {
    serde_json::from_slice(bytes).map_err(|error| {
        let location = if error.is_data() {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|document| {
                    argument_error_location(&document, include_patches)
                })
        } else {
            None
        };
        version_json_error(path, error, location.as_deref(), false)
    })
}

pub(super) fn parse_merged_version_json<T: DeserializeOwned>(
    path: &Path,
    document: &Value,
) -> crate::Result<T> {
    T::deserialize(document).map_err(|error| {
        let location = argument_error_location(document, true);
        version_json_error(path, error, location.as_deref(), true)
    })
}

fn version_json_error(
    path: &Path,
    error: serde_json::Error,
    location: Option<&str>,
    merged: bool,
) -> crate::Error {
    let document = if merged {
        "merged version JSON"
    } else {
        "version JSON"
    };
    let detail = match (location, error.classify()) {
        (Some(location), _) => format!("invalid arguments at {location}"),
        (None, serde_json::error::Category::Eof) => {
            "unexpected end of JSON".into()
        }
        (None, serde_json::error::Category::Syntax) => {
            "invalid JSON syntax".into()
        }
        (None, _) => "invalid version metadata".into(),
    };
    let message =
        format!("Failed to parse {document} {}: {detail}", path.display());
    let message = if error.line() != 0 {
        format!(
            "{message} (line {}, column {})",
            error.line(),
            error.column()
        )
    } else {
        message
    };
    crate::ErrorKind::JSONError(serde::de::Error::custom(message)).into()
}

fn argument_error_location(
    document: &Value,
    include_patches: bool,
) -> Option<String> {
    if let Some(location) = invalid_arguments(document) {
        return Some(location);
    }
    if include_patches
        && let Some(patches) = document.get("patches").and_then(Value::as_array)
    {
        for (index, patch) in patches.iter().enumerate() {
            if let Some(location) = invalid_arguments(patch) {
                return Some(format!("patches[{index}].{location}"));
            }
        }
    }
    None
}

fn invalid_arguments(document: &Value) -> Option<String> {
    if document
        .get("minecraftArguments")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Some("minecraftArguments".into());
    }
    let arguments =
        document.get("arguments").filter(|value| !value.is_null())?;
    let Some(groups) = arguments.as_object() else {
        return Some("arguments".into());
    };
    for (group, values) in groups {
        let name =
            serde::de::value::StrDeserializer::<serde_json::Error>::new(group);
        if ArgumentType::deserialize(name).is_err() {
            return Some("arguments (unknown group)".into());
        }
        let Some(values) = values.as_array() else {
            return Some(format!("arguments.{group}"));
        };
        for (index, value) in values.iter().enumerate() {
            if Argument::deserialize(value).is_err() {
                return Some(format!("arguments.{group}[{index}]"));
            }
        }
    }
    None
}
