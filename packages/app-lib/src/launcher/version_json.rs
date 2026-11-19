use std::path::Path;

use daedalus::minecraft::{
    Argument, ArgumentType, ArgumentValue, FeatureRule, OsRule, Rule,
    RuleAction,
};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

struct ArgumentError {
    location: String,
    expected: &'static str,
}

pub(super) fn parse_version_json<T: DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
    include_patches: bool,
) -> crate::Result<T> {
    serde_json::from_slice(bytes).map_err(|error| {
        let argument = if error.is_data() {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|document| argument_error(&document, include_patches))
        } else {
            None
        };
        version_json_error(path, error, argument, false)
    })
}

pub(super) fn parse_merged_version_json<T: DeserializeOwned>(
    path: &Path,
    document: &Value,
) -> crate::Result<T> {
    T::deserialize(document).map_err(|error| {
        version_json_error(path, error, argument_error(document, true), true)
    })
}

fn version_json_error(
    path: &Path,
    error: serde_json::Error,
    argument_error: Option<ArgumentError>,
    merged: bool,
) -> crate::Error {
    let document = if merged {
        "merged version JSON"
    } else {
        "version JSON"
    };
    let detail = match (argument_error, error.classify()) {
        (Some(ArgumentError { location, expected }), _) => {
            format!("invalid arguments at {location}; expected {expected}")
        }
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

fn argument_error(
    document: &Value,
    include_patches: bool,
) -> Option<ArgumentError> {
    if let Some(error) = invalid_arguments(document) {
        return Some(error);
    }
    if include_patches
        && let Some(patches) = document.get("patches").and_then(Value::as_array)
    {
        for (index, patch) in patches.iter().enumerate() {
            if let Some(mut error) = invalid_arguments(patch) {
                error.location = format!("patches[{index}].{}", error.location);
                return Some(error);
            }
        }
    }
    None
}

fn invalid_arguments(document: &Value) -> Option<ArgumentError> {
    if document
        .get("minecraftArguments")
        .is_some_and(|value| !value.is_null() && !value.is_string())
    {
        return Some(ArgumentError {
            location: "minecraftArguments".into(),
            expected: "a string",
        });
    }
    let arguments =
        document.get("arguments").filter(|value| !value.is_null())?;
    let Some(groups) = arguments.as_object() else {
        return Some(ArgumentError {
            location: "arguments".into(),
            expected: "an object of argument groups",
        });
    };
    for (group, values) in groups {
        let name =
            serde::de::value::StrDeserializer::<serde_json::Error>::new(group);
        if ArgumentType::deserialize(name).is_err() {
            return Some(ArgumentError {
                location: "arguments (unknown group)".into(),
                expected: "game, jvm or default-user-jvm",
            });
        }
        let Some(values) = values.as_array() else {
            return Some(ArgumentError {
                location: format!("arguments.{group}"),
                expected: "an array of arguments",
            });
        };
        for (index, value) in values.iter().enumerate() {
            if Argument::deserialize(value).is_err() {
                return Some(invalid_argument(
                    value,
                    format!("arguments.{group}[{index}]"),
                ));
            }
        }
    }
    None
}

fn invalid_argument(argument: &Value, location: String) -> ArgumentError {
    let Some(fields) = argument.as_object() else {
        return ArgumentError {
            location,
            expected: "a string or an argument object",
        };
    };
    if !fields
        .get("value")
        .is_some_and(|value| ArgumentValue::deserialize(value).is_ok())
    {
        return ArgumentError {
            location: format!("{location}.value"),
            expected: "a string or an array of strings",
        };
    }
    if let Some(rules) = fields.get("rules") {
        let Some(rules) = rules.as_array() else {
            return ArgumentError {
                location: format!("{location}.rules"),
                expected: "an array of rules",
            };
        };
        for (index, rule) in rules.iter().enumerate() {
            if Rule::deserialize(rule).is_ok() {
                continue;
            }
            let location = format!("{location}.rules[{index}]");
            let (field, expected) =
                if !rule.is_object() {
                    ("", "a rule object")
                } else if !rule.get("action").is_some_and(|action| {
                    RuleAction::deserialize(action).is_ok()
                }) {
                    (".action", "allow or disallow")
                } else if rule.get("os").is_some_and(|os| {
                    Option::<OsRule>::deserialize(os).is_err()
                }) {
                    (".os", "a valid operating-system rule")
                } else if rule.get("features").is_some_and(|features| {
                    Option::<FeatureRule>::deserialize(features).is_err()
                }) {
                    (".features", "an object with boolean feature flags")
                } else {
                    ("", "a valid rule object")
                };
            return ArgumentError {
                location: format!("{location}{field}"),
                expected,
            };
        }
    }
    ArgumentError {
        location,
        expected: "a valid argument object",
    }
}
