use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use serde_json::{Map, Value};

use super::pcl_ce_arguments;

static VERSION_COMPONENT: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new("[a-z]+|[0-9]+").unwrap());

pub(super) fn normalize(
    document: &mut Value,
    path: &Path,
) -> crate::Result<()> {
    normalize_for_host(
        document,
        path,
        pcl_ce_arguments::windows_architecture_width(),
    )
}

fn normalize_for_host(
    document: &mut Value,
    path: &Path,
    windows_width: Option<&str>,
) -> crate::Result<()> {
    let Some(libraries) = document.get_mut("libraries") else {
        return Ok(());
    };
    let Value::Array(originals) = std::mem::take(libraries) else {
        *libraries = Value::Array(Vec::new());
        return Ok(());
    };
    let mut selected = Vec::<(String, Value)>::new();
    let mut slots = HashMap::new();
    for (index, library) in originals.into_iter().enumerate() {
        let Value::Object(mut fields) = library else {
            continue;
        };
        if !pcl_ce_arguments::allows_library(fields.get("rules"), path, index)?
        {
            continue;
        }
        fields.remove("rules");
        fields.retain(|_, value| !value.is_null());
        let is_native = fields.contains_key("natives");
        if is_native {
            let Some(width) = windows_width else {
                selected.push((String::new(), Value::Object(fields)));
                continue;
            };
            if !project_windows_native(&mut fields, width) {
                continue;
            }
        }
        let candidate = Value::Object(fields);
        let Some((identity, version)) = library_identity(&candidate) else {
            selected.push((String::new(), candidate));
            continue;
        };
        let version = version.to_owned();
        let slot = slots.entry((identity, is_native)).or_insert(selected.len());
        if *slot == selected.len() {
            selected.push((version, candidate));
        } else if compare_versions(&version, &selected[*slot].0)
            != Ordering::Less
        {
            selected[*slot] = (version, candidate);
        }
    }
    *libraries = Value::Array(
        selected.into_iter().map(|(_, library)| library).collect(),
    );
    Ok(())
}

fn project_windows_native(
    fields: &mut Map<String, Value>,
    width: &str,
) -> bool {
    let Some(classifier) = fields
        .get("natives")
        .and_then(|natives| natives.get("windows"))
        .and_then(Value::as_str)
        .map(|classifier| classifier.replace("${arch}", width))
    else {
        return false;
    };
    fields.insert("include_in_classpath".into(), Value::Bool(false));
    fields.insert(
        "natives".into(),
        serde_json::json!({ "windows": classifier }),
    );
    if let Some(downloads) =
        fields.get_mut("downloads").and_then(Value::as_object_mut)
    {
        downloads.remove("artifact");
        if let Some(classifiers) = downloads
            .get_mut("classifiers")
            .and_then(Value::as_object_mut)
        {
            let artifact = ["natives-windows", classifier.as_str()]
                .into_iter()
                .find_map(|name| {
                    classifiers
                        .iter()
                        .find(|(key, value)| {
                            !value.is_null() && key.eq_ignore_ascii_case(name)
                        })
                        .map(|(_, value)| value.clone())
                });
            if let Some(artifact) = artifact {
                classifiers.insert(classifier, artifact);
            }
        }
    }
    true
}

fn library_identity(library: &Value) -> Option<(String, &str)> {
    let coordinate = library.get("name")?.as_str()?;
    let (prefix, version_and_tail) = coordinate.split_once(':')?;
    let (artifact, version_and_tail) = version_and_tail.split_once(':')?;
    let (version, tail) = version_and_tail
        .split_once(':')
        .unwrap_or((version_and_tail, ""));
    let identity = if tail.is_empty() {
        format!("{prefix}:{artifact}")
    } else {
        format!("{prefix}:{artifact}:{tail}")
    };
    let downloads = library.get("downloads");
    let native = library.get("natives").is_some();
    let file = if native {
        library
            .get("natives")
            .and_then(|natives| natives.get("windows"))
            .and_then(Value::as_str)
            .and_then(|classifier| {
                downloads?.get("classifiers")?.get(classifier)
            })
    } else {
        downloads
            .and_then(|downloads| downloads.get("artifact"))
            .filter(|artifact| !artifact.is_null())
    };
    let version = if let Some(path) = file
        .and_then(|file| file.get("path"))
        .and_then(Value::as_str)
    {
        path.rsplit(['/', '\\']).nth(1).unwrap_or("libraries")
    } else if !native
        && file.is_none()
        && library.get("hint").and_then(Value::as_str) == Some("local")
    {
        "libraries"
    } else {
        version.split('@').next().unwrap_or_default()
    };
    Some((identity, version))
}

fn compare_versions(left: &str, right: &str) -> Ordering {
    let unknown = |version: &str| {
        version.is_empty() || version.eq_ignore_ascii_case("unknown")
    };
    match (unknown(left), unknown(right)) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }

    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let normalize = |version: &str| {
        version.replace("快照", "snapshot").replace("预览版", "pre")
    };
    let left_tokens = normalize(&left);
    let right_tokens = normalize(&right);
    let mut left_components = VERSION_COMPONENT.find_iter(&left_tokens);
    let mut right_components = VERSION_COMPONENT.find_iter(&right_tokens);
    loop {
        let pair = (left_components.next(), right_components.next());
        if matches!(pair, (None, None)) {
            return left.encode_utf16().cmp(right.encode_utf16());
        }
        let left = pair.0.map_or("0", |token| token.as_str());
        let right = pair.1.map_or("0", |token| token.as_str());
        let left_value = version_component_value(left);
        let right_value = version_component_value(right);
        let ordering = if left_value == 0.0 && right_value == 0.0 {
            left.cmp(right)
        } else {
            left_value.total_cmp(&right_value)
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn version_component_value(token: &str) -> f64 {
    match token {
        "rc" => -1.0,
        "pre" => -2.0,
        "snapshot" => -3.0,
        "experimental" => -4.0,
        _ => token
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn filters_rules_before_typed_decoding_and_uses_the_pcl_ce_tie_break() {
        let mut document = json!({"libraries": [
            false,
            {"name":"example:module:1.10", "url":"https://earlier.test", "rules":null},
            {"name":"example:other:2"},
            {"name":"example:module:1.9"},
            {"name":"example:module:1.10-pre2"},
            {"name":"example:module:1.10", "url":"https://later.test", "rules":[null,{"action":"allow","os":{"name":"unknown"}}]},
            {"name":"example:module:99", "downloads":false, "rules":[]},
            {"name":"example:module:100", "rules":[{"action":"allow"},{"action":"DENY"}]},
            {"name":"example:module:4:natives-windows"},
        ]});
        normalize_for_host(&mut document, Path::new("client.json"), None)
            .unwrap();
        assert_eq!(
            document["libraries"],
            json!([
                {"name":"example:module:1.10", "url":"https://later.test"},
                {"name":"example:other:2"},
                {"name":"example:module:4:natives-windows"},
            ])
        );
    }

    #[test]
    fn uses_declared_path_versions_and_keeps_native_and_classpath_slots_separate()
     {
        let mut document = json!({"libraries":[
            {"name":"example:module:4", "downloads":{"artifact":{"path":"example/module/11/module.jar", "sha1":"winner"}}},
            {"name":"example:module:9"},
            {"name":"example:module:15", "natives":{"linux":"natives-linux"}},
            {"name":"example:module:2", "natives":{"windows":"natives-windows-${arch}", "linux":"natives-linux"}, "downloads":{"artifact":{"path":"example/module/2/unused.jar"}, "classifiers":{"Natives-Windows":{"path":"example/module/10/native.jar","sha1":"native-winner","size":123}}}},
            {"name":"example:module:3", "natives":{"windows":"natives-windows"}},
        ]});
        normalize_for_host(&mut document, Path::new("client.json"), Some("64"))
            .unwrap();
        let libraries = document["libraries"].as_array().unwrap();
        assert_eq!(libraries.len(), 2);
        assert_eq!(libraries[0]["name"], "example:module:4");
        assert_eq!(libraries[0]["downloads"]["artifact"]["sha1"], "winner");
        assert_eq!(libraries[1]["name"], "example:module:2");
        assert_eq!(
            libraries[1]["natives"],
            json!({"windows":"natives-windows-64"})
        );
        assert_eq!(libraries[1]["include_in_classpath"], false);
        assert!(libraries[1]["downloads"].get("artifact").is_none());
        assert_eq!(
            libraries[1]["downloads"]["classifiers"]["natives-windows-64"],
            json!({"path":"example/module/10/native.jar","sha1":"native-winner","size":123}),
        );
    }

    #[test]
    fn invalid_rules_report_the_library_index_without_its_contents() {
        let mut document = json!({"libraries":[
            {"name":"example:module:1"},
            {"name":"example:private-name:1", "rules":[{"features":"private-value"}]},
        ]});
        let error = normalize(&mut document, Path::new("client.json"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("libraries[1].rules[0].features"), "{error}");
        assert!(!error.contains("private"), "{error}");
    }

    #[test]
    fn non_windows_keeps_native_classifier_records_for_runtime_selection() {
        let libraries = json!([
            {"name":"example:module:4", "natives":{"linux":"natives-linux"}},
            {"name":"example:module:8", "natives":{"osx":"natives-osx"}},
            {"name":"example:module:2", "natives":{"linux-arm64":"natives-linux-arm64"}},
        ]);
        let mut document = json!({"libraries": libraries});
        normalize_for_host(&mut document, Path::new("client.json"), None)
            .unwrap();
        assert_eq!(document["libraries"], libraries);
    }

    #[test]
    fn implements_pcl_ce_version_ordering() {
        for (less, greater) in [
            ("1.9", "1.10"),
            ("1.10-rc5", "1.10"),
            ("1.10-pre10", "1.10-rc1"),
            ("26.4-experimental7", "26.4-snapshot1"),
            ("26.4-快照8", "26.4-pre1"),
            ("26.4", "Unknown"),
            ("0", ""),
        ] {
            assert_eq!(
                compare_versions(less, greater),
                Ordering::Less,
                "{less} < {greater}"
            );
        }
        assert_eq!(compare_versions("1.20-RC1", "1.20-rc1"), Ordering::Equal);
    }

    #[test]
    fn linked_dialect_publishes_a_shared_selected_library_snapshot() {
        use crate::launcher::direct_link::LinkedLauncherDialect;
        use crate::launcher::local_version::load_version_for_dialect;

        let root = tempfile::tempdir().unwrap();
        for (id, content) in [
            (
                "base",
                json!({"id":"base", "libraries":[{"name":"example:module:1.10"},{"name":"example:other:1", "rules":[]}]}),
            ),
            (
                "child",
                json!({"id":"child", "inheritsFrom":"base", "libraries":[{"name":"example:module:1.9"}]}),
            ),
        ] {
            let folder = root.path().join("versions").join(id);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join(format!("{id}.json")),
                content.to_string(),
            )
            .unwrap();
        }
        let pcl_ce = load_version_for_dialect(
            root.path(),
            "child",
            None,
            LinkedLauncherDialect::PclCe,
        )
        .unwrap();
        let generic = load_version_for_dialect(
            root.path(),
            "child",
            None,
            LinkedLauncherDialect::Generic,
        )
        .unwrap();
        assert_eq!(pcl_ce.libraries.len(), 1);
        assert_eq!(pcl_ce.libraries[0].library.name, "example:module:1.10");
        assert!(pcl_ce.libraries[0].library.rules.is_none());
        assert!(
            generic
                .libraries
                .iter()
                .any(|library| library.library.name == "example:other:1")
        );
    }
}
