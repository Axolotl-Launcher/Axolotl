use serde::Deserialize;
use std::collections::HashMap;

/// Fabric mod.json (and Quilt's similar quilt.mod.json wrapped under quilt_loader).
#[derive(Debug, Deserialize)]
pub(crate) struct FabricModJson {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<FabricAuthorOrArray>,
    #[serde(default)]
    pub contributors: Vec<FabricAuthorOrArray>,
    pub icon: Option<ModIcon>,
    #[serde(rename = "contact")]
    pub _contact: Option<serde_json::Value>,
    /// Dependency resolution (Fabric: map of id→version, Quilt: array of objects).
    /// Uses `serde_json::Value` to handle both formats.
    pub depends: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub recommends: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub conflicts: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub breaks: Option<serde_json::Value>,
}

/// Fabric's `icon` field accepts either a plain path or a dictionary mapping
/// icon size to path.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ModIcon {
    Path(String),
    Sized(HashMap<String, String>),
}

impl ModIcon {
    /// Resolve a single icon path, preferring the largest declared size.
    pub(crate) fn resolve(&self) -> Option<String> {
        match self {
            Self::Path(path) => Some(path.clone()),
            Self::Sized(sizes) => sizes
                .iter()
                .max_by_key(|(size, _)| size.parse::<u64>().unwrap_or(0))
                .map(|(_, path)| path.clone()),
        }
    }
}

/// An author/contributor entry: either a plain string or `{"name": "...", "contact": {...}}`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum FabricAuthorOrArray {
    Plain(String),
    Object { name: Option<String> },
}

/// Quilt's wrapper: `{"quilt_loader": { "id": "...", ... }}`.
#[derive(Debug, Deserialize)]
pub(crate) struct QuiltModJson {
    pub quilt_loader: FabricModJson,
}

/// Extract a string value from a Fabric-style depends map.
pub(crate) fn fabric_dep_value(
    depends: &Option<serde_json::Value>,
    key: &str,
) -> Option<String> {
    let obj = depends.as_ref()?.as_object()?;
    obj.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(arr) => {
            let versions = arr
                .iter()
                .map(|value| value.as_str())
                .collect::<Option<Vec<_>>>()?;
            (!versions.is_empty()).then(|| versions.join(" || "))
        }
        _ => None,
    })
}

/// Extract a string value from a Quilt-style depends array.
pub(crate) fn quilt_dep_value(
    depends: &Option<serde_json::Value>,
    key: &str,
) -> Option<String> {
    let arr = depends.as_ref()?.as_array()?;
    for dep in arr {
        let id = dep
            .as_object()
            .and_then(|obj| obj.get("id"))
            .and_then(|v| v.as_str())?;
        if id == key {
            return dep
                .as_object()
                .and_then(|obj| obj.get("versions"))
                .and_then(|v| v.as_str())
                .map(String::from);
        }
    }
    None
}

/// Split a `"modid: version-range"` string into its parts.
fn split_dependency_id(text: &str) -> (String, Option<String>) {
    match text.split_once(':') {
        Some((id, range)) => {
            (id.trim().to_string(), Some(range.trim().to_string()))
        }
        None => (text.trim().to_string(), None),
    }
}

fn push_dependency(
    out: &mut Vec<super::LocalModDependency>,
    mod_id: String,
    version_range: Option<String>,
) {
    if super::is_env_dependency_id(&mod_id) {
        return;
    }
    out.push(super::LocalModDependency {
        mod_id,
        version_range,
    })
}

/// Extract required dependencies from a Fabric-style `depends` map
/// (`"id": "range"` or `"id": ["range alternative", "range alternative"]`).
pub(crate) fn fabric_dependencies(
    depends: &Option<serde_json::Value>,
) -> Vec<super::LocalModDependency> {
    let Some(obj) = depends.as_ref().and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (id, value) in obj {
        match value {
            serde_json::Value::String(range) => {
                push_dependency(&mut out, id.clone(), Some(range.clone()));
            }
            serde_json::Value::Array(alternatives) => {
                if let Some(versions) = alternatives
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Option<Vec<_>>>()
                    && !versions.is_empty()
                {
                    push_dependency(
                        &mut out,
                        id.clone(),
                        Some(versions.join(" || ")),
                    );
                }
            }
            _ => {}
        }
    }
    out
}

/// Extract required dependencies from a Quilt-style `depends` array
/// (`[{"id": "...", "versions": "..."}]`, nested arrays list alternatives).
pub(crate) fn quilt_dependencies(
    depends: &Option<serde_json::Value>,
) -> Vec<super::LocalModDependency> {
    let Some(arr) = depends.as_ref().and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in arr {
        match entry {
            serde_json::Value::String(text) => {
                let (mod_id, version_range) = split_dependency_id(text);
                push_dependency(&mut out, mod_id, version_range);
            }
            serde_json::Value::Object(obj) => {
                let Some(mod_id) = obj.get("id").and_then(|v| v.as_str())
                else {
                    continue;
                };
                let version_range = obj
                    .get("versions")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                push_dependency(&mut out, mod_id.to_string(), version_range);
            }
            serde_json::Value::Array(alternatives) => {
                for alternative in alternatives {
                    let Some(obj) = alternative.as_object() else {
                        continue;
                    };
                    let Some(mod_id) = obj.get("id").and_then(|v| v.as_str())
                    else {
                        continue;
                    };
                    let version_range = obj
                        .get("versions")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    push_dependency(
                        &mut out,
                        mod_id.to_string(),
                        version_range,
                    );
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabric_dependencies_keep_project_ids_and_version_alternatives() {
        let depends = serde_json::json!({
            "sodium": ">=0.4.10",
            "fabricloader": ">=0.15.0",
            "minecraft": ["~1.20.1", "~1.21.1"],
            "physics": [">=1.2", "~2.0"]
        });
        let deps = fabric_dependencies(&Some(depends.clone()));
        let mut ids = deps
            .iter()
            .map(|dep| dep.mod_id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, ["physics", "sodium"]);
        let sodium = deps.iter().find(|dep| dep.mod_id == "sodium").unwrap();
        assert_eq!(sodium.version_range.as_deref(), Some(">=0.4.10"));
        let physics = deps.iter().find(|dep| dep.mod_id == "physics").unwrap();
        assert_eq!(physics.version_range.as_deref(), Some(">=1.2 || ~2.0"));
        assert_eq!(
            fabric_dep_value(&Some(depends), "minecraft").as_deref(),
            Some("~1.20.1 || ~1.21.1")
        );
    }

    #[test]
    fn quilt_dependencies_handle_objects_strings_and_alternatives() {
        let depends = serde_json::json!([
            { "id": "sodium", "versions": "*" },
            "indium",
            [
                { "id": "canvas", "versions": ">=1.0" },
                { "id": "minecraft" }
            ]
        ]);
        let deps = quilt_dependencies(&Some(depends));
        let ids = deps
            .iter()
            .map(|dep| dep.mod_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["sodium", "indium", "canvas"]);
        assert_eq!(deps[2].version_range.as_deref(), Some(">=1.0"));
    }
}
