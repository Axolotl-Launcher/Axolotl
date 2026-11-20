use std::path::Path;
use std::sync::LazyLock;

use serde_json::{Map, Value};

struct Platform {
    windows: bool,
    windows_version: Option<String>,
    is_32_bit: bool,
}

static PLATFORM: LazyLock<Platform> = LazyLock::new(|| Platform {
    windows: cfg!(windows),
    windows_version: windows_version(),
    is_32_bit: is_32_bit_system(),
});

#[cfg(windows)]
fn windows_version() -> Option<String> {
    use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok()?;
    let major: u32 = key.get_value("CurrentMajorVersionNumber").ok()?;
    let minor: u32 = key.get_value("CurrentMinorVersionNumber").ok()?;
    let build: String = key.get_value("CurrentBuildNumber").ok()?;
    let build = build.parse::<u32>().ok()?;
    Some(format!("{major}.{minor}.{build}.0"))
}

#[cfg(not(windows))]
fn windows_version() -> Option<String> {
    None
}

#[cfg(all(windows, target_pointer_width = "32"))]
fn is_32_bit_system() -> bool {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, IsWow64Process,
    };

    let mut wow64 = windows_core::BOOL(0);
    unsafe { IsWow64Process(GetCurrentProcess(), &mut wow64) }
        .map_or(true, |()| !wow64.as_bool())
}

#[cfg(not(all(windows, target_pointer_width = "32")))]
fn is_32_bit_system() -> bool {
    cfg!(target_pointer_width = "32")
}

/// Project PCL-CE's dynamic launch arguments into Axolotl's typed model.
/// Rule selection happens before reading values; retained argument objects
/// keep their shape so the existing tokenization path is not changed here.
pub(super) fn normalize(
    document: &mut Value,
    path: &Path,
) -> crate::Result<()> {
    normalize_for_platform(document, path, &PLATFORM)
}

fn normalize_for_platform(
    document: &mut Value,
    path: &Path,
    platform: &Platform,
) -> crate::Result<()> {
    if let Some(legacy) = document.get_mut("minecraftArguments")
        && !legacy.is_null()
        && !legacy.is_string()
    {
        *legacy = Value::String(node_text(legacy));
    }
    let Some(arguments) = document
        .get_mut("arguments")
        .filter(|arguments| !arguments.is_null())
    else {
        return Ok(());
    };
    let groups = arguments.as_object().ok_or_else(|| {
        invalid(path, "arguments", "an object of argument groups")
    })?;
    let mut normalized_groups = Map::new();
    for group in ["game", "jvm"] {
        let Some(values) = groups.get(group).filter(|values| !values.is_null())
        else {
            continue;
        };
        let values = values.as_array().ok_or_else(|| {
            invalid(path, &format!("arguments.{group}"), "an argument array")
        })?;
        let mut normalized = Vec::with_capacity(values.len());
        for (index, argument) in values.iter().enumerate() {
            if argument.is_string() {
                normalized.push(argument.clone());
                continue;
            }
            let location = format!("arguments.{group}[{index}]");
            let fields = argument.as_object().ok_or_else(|| {
                invalid(path, &location, "a string or an argument object")
            })?;
            if allows_argument(fields.get("rules"), platform).map_err(
                |error| {
                    invalid(
                        path,
                        &format!("{location}{}", error.field),
                        error.expected,
                    )
                },
            )? {
                let value =
                    normalize_value(fields.get("value"), path, &location)?;
                normalized.push(Value::Object(Map::from_iter([(
                    "value".into(),
                    value,
                )])));
            }
        }
        normalized_groups.insert(group.into(), Value::Array(normalized));
    }
    *arguments = Value::Object(normalized_groups);
    Ok(())
}

fn normalize_value(
    value: Option<&Value>,
    path: &Path,
    location: &str,
) -> crate::Result<Value> {
    match value {
        Some(value @ Value::String(_)) => Ok(value.clone()),
        Some(Value::Array(values)) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if value.is_null() {
                    return Err(invalid(
                        path,
                        &format!("{location}.value[{index}]"),
                        "a non-null argument value",
                    ));
                }
                Ok(Value::String(node_text(value)))
            })
            .collect::<crate::Result<Vec<_>>>()
            .map(Value::Array),
        _ => Err(invalid(
            path,
            &format!("{location}.value"),
            "a string or an array of argument values",
        )),
    }
}

fn node_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(_) | Value::Array(_) => {
            serde_json::to_string_pretty(value)
                .unwrap_or_else(|_| value.to_string())
        }
        _ => value.to_string(),
    }
}

struct RuleError {
    field: String,
    expected: &'static str,
}

fn allows_argument(
    rules: Option<&Value>,
    platform: &Platform,
) -> Result<bool, RuleError> {
    let Some(rules) = rules.filter(|rules| !rules.is_null()) else {
        return Ok(true);
    };
    let rules = rules.as_array().ok_or_else(|| RuleError {
        field: ".rules".into(),
        expected: "an array of rules",
    })?;
    let mut allowed = false;
    for (index, rule) in rules.iter().enumerate() {
        if rule.is_null() {
            continue;
        }
        let rule = rule.as_object().ok_or_else(|| RuleError {
            field: format!(".rules[{index}]"),
            expected: "a rule object",
        })?;
        let os_matches = optional_rule_object(rule, "os", index)?
            .is_none_or(|os| matches_os(os, platform));
        let features_match = optional_rule_object(rule, "features", index)?
            .is_none_or(matches_features);
        if os_matches && features_match {
            allowed =
                rule.get("action").and_then(Value::as_str) == Some("allow");
        }
    }
    Ok(allowed)
}

fn optional_rule_object<'a>(
    rule: &'a Map<String, Value>,
    field: &str,
    index: usize,
) -> Result<Option<&'a Map<String, Value>>, RuleError> {
    rule.get(field)
        .filter(|value| !value.is_null())
        .map(|value| {
            value.as_object().ok_or_else(|| RuleError {
                field: format!(".rules[{index}].{field}"),
                expected: "an object",
            })
        })
        .transpose()
}

fn matches_os(os: &Map<String, Value>, platform: &Platform) -> bool {
    let name_matches = match os.get("name").filter(|name| !name.is_null()) {
        None => true,
        Some(name) => match name.as_str() {
            Some("unknown") => true,
            Some("windows") if platform.windows => os
                .get("version")
                .filter(|version| !version.is_null())
                .is_none_or(|pattern| {
                    platform.windows_version.as_deref().is_some_and(|version| {
                        regex::Regex::new(&node_text(pattern))
                            .is_ok_and(|pattern| pattern.is_match(version))
                    })
                }),
            _ => false,
        },
    };
    name_matches
        && os
            .get("arch")
            .filter(|arch| !arch.is_null())
            .is_none_or(|arch| {
                (arch.as_str() == Some("x86")) == platform.is_32_bit
            })
}

fn matches_features(features: &Map<String, Value>) -> bool {
    !features.iter().any(|(name, value)| {
        (name.eq_ignore_ascii_case("is_demo_user") && !value.is_null())
            || name.contains("quick_play")
    })
}

fn invalid(path: &Path, location: &str, expected: &str) -> crate::Error {
    crate::ErrorKind::JSONError(serde::de::Error::custom(format!(
        "Failed to parse merged version JSON {}: invalid arguments at {location}; expected {expected}",
        path.display()
    )))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn windows_platform() -> Platform {
        Platform {
            windows: true,
            windows_version: Some("10.0.22631.0".into()),
            is_32_bit: false,
        }
    }

    fn normalize_fixture(document: &mut Value) -> crate::Result<()> {
        normalize_for_platform(
            document,
            Path::new("client.json"),
            &windows_platform(),
        )
    }

    #[test]
    fn normalizes_known_groups_and_evaluates_rules_before_reading_values() {
        let mut document = json!({
            "minecraftArguments": "--legacy",
            "arguments": {
                "game": [
                    "--plain",
                    {"value": "--implicit"},
                    {"value": "--null-rules", "rules": null},
                    {"value": "--not-emitted", "rules": []},
                    {"rules": [{"action": "allow"}, {"action": "Allow"}]},
                    {"rules": [{}], "value": 123},
                    {"rules": [null, {"action": "allow"}], "value": ["--coerced", 10, true]},
                ],
                "jvm": null,
                "extra": [null, 5],
            },
        });
        normalize_fixture(&mut document).unwrap();
        assert_eq!(
            document,
            json!({
                "minecraftArguments": "--legacy",
                "arguments": {"game": [
                    "--plain",
                    {"value": "--implicit"},
                    {"value": "--null-rules"},
                    {"value": ["--coerced", "10", "true"]},
                ]},
            })
        );
    }

    #[test]
    fn preserves_legacy_and_empty_jvm_group_semantics() {
        let mut document = json!({
            "minecraftArguments": false,
            "arguments": {"jvm": [{"rules": [{"action": "disallow"}]}]},
        });
        normalize_fixture(&mut document).unwrap();
        assert_eq!(
            document,
            json!({"minecraftArguments": "false", "arguments": {"jvm": []}})
        );
    }

    #[test]
    fn applies_pcl_ce_os_and_feature_checks() {
        let mut document = json!({"arguments": {"jvm": [
            {"value": "matched-version", "rules": [{"action": "allow", "os": {"name": "windows", "version": "^10\\.0\\.22", "arch": "amd64"}}]},
            {"value": "wrong-version", "rules": [{"action": "allow", "os": {"name": "windows", "version": "^6\\."}}]},
            {"value": "invalid-regex", "rules": [{"action": "allow", "os": {"name": "windows", "version": "["}}]},
            {"value": "wrong-width", "rules": [{"action": "allow", "os": {"arch": "x86"}}]},
            {"value": "pcl-unknown", "rules": [{"action": "allow", "os": {"name": "unknown", "version": "never"}}]},
            {"value": "not-a-pcl-platform", "rules": [{"action": "allow", "os": {"name": "linux"}}]},
            {"value": "demo-false", "rules": [{"action": "allow", "features": {"IS_DEMO_USER": false}}]},
            {"value": "demo-null", "rules": [{"action": "allow", "features": {"is_demo_user": null, "has_custom_resolution": false}}]},
            {"value": "quick-false", "rules": [{"action": "allow", "features": {"is_quick_play_singleplayer": false}}]},
            {"value": "quick-key-case", "rules": [{"action": "allow", "features": {"IS_QUICK_PLAY_SINGLEPLAYER": true}}]},
        ]}});
        normalize_fixture(&mut document).unwrap();
        assert_eq!(
            document["arguments"]["jvm"],
            json!([
                {"value": "matched-version"},
                {"value": "pcl-unknown"},
                {"value": "demo-null"},
                {"value": "quick-key-case"},
            ])
        );
    }

    #[test]
    fn rejects_invalid_parameter_shapes_without_disclosing_values() {
        for (arguments, location) in [
            (json!(["private-value"]), "arguments"),
            (json!({"game": "private-value"}), "arguments.game"),
            (json!({"game": [10, "private-value"]}), "arguments.game[0]"),
            (
                json!({"game": [null, "private-value"]}),
                "arguments.game[0]",
            ),
            (
                json!({"game": [{"value": 10}, "private-value"]}),
                "arguments.game[0].value",
            ),
            (
                json!({"game": [{"value": ["private-value", null]}]}),
                "arguments.game[0].value[1]",
            ),
            (
                json!({"game": [{"rules": "private-value"}]}),
                "arguments.game[0].rules",
            ),
            (
                json!({"game": [{"rules": [null, "private-value"]}]}),
                "arguments.game[0].rules[1]",
            ),
            (
                json!({"game": [{"rules": [{"action": "disallow", "features": "private-value"}]}]}),
                "arguments.game[0].rules[0].features",
            ),
            (
                json!({"game": [{"rules": [{"action": "disallow", "os": "private-value"}]}]}),
                "arguments.game[0].rules[0].os",
            ),
        ] {
            let mut document = json!({"arguments": arguments});
            let error =
                normalize_fixture(&mut document).unwrap_err().to_string();
            assert!(error.contains(&format!("at {location};")), "{error}");
            assert!(error.contains("client.json"), "{error}");
            assert!(!error.contains("private-value"), "{error}");
        }
    }
}
