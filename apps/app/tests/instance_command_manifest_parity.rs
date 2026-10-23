use std::collections::BTreeSet;

fn bracket_contents_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing source marker: {marker}"))
        + marker.len();
    let mut depth = 1_u32;
    for (offset, character) in source[start..].char_indices() {
        match character {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated command list after marker: {marker}");
}

fn runtime_commands(source: &str) -> BTreeSet<&str> {
    bracket_contents_after(source, "tauri::generate_handler![")
        .split(',')
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(|command| command.rsplit("::").next().unwrap())
        .collect()
}

fn manifest_commands<'a>(source: &'a str, plugin: &str) -> BTreeSet<&'a str> {
    let instance_plugin = source
        .match_indices(".plugin(")
        .find(|(index, _)| {
            source[index + ".plugin(".len()..]
                .trim_start()
                .starts_with(&format!("\"{plugin}\","))
        })
        .map(|(index, _)| index)
        .expect("missing InlinedPlugin");
    bracket_contents_after(&source[instance_plugin..], ".commands(&[")
        .split(',')
        .map(str::trim)
        .filter_map(|command| command.strip_prefix('"')?.strip_suffix('"'))
        .collect()
}

#[test]
fn instance_runtime_handlers_are_registered_in_build_manifest() {
    let runtime = runtime_commands(include_str!("../src/api/instance.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "instance");
    let missing = runtime.difference(&manifest).copied().collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "instance commands missing from build.rs manifest: {}",
        missing.join(", ")
    );
    for command in [
        "instance_discard_upgrade_plan",
        "instance_get_post_upgrade_notice",
        "instance_dismiss_post_upgrade_notice",
    ] {
        assert!(
            runtime.contains(command),
            "runtime handler missing {command}"
        );
        assert!(
            manifest.contains(command),
            "build.rs manifest missing {command}"
        );
    }
}

#[test]
fn server_log_delta_is_registered_and_allowed_for_the_main_window() {
    let runtime = runtime_commands(include_str!("../src/api/servers.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "servers");
    assert!(runtime.contains("servers_get_log_delta"));
    assert!(manifest.contains("servers_get_log_delta"));
    assert!(runtime.difference(&manifest).next().is_none());
    let capabilities: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert!(
        capabilities["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| window == "main")
    );
    assert!(
        capabilities["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "servers:default")
    );
}

#[test]
fn mojang_monitor_commands_are_registered_and_allowed_for_the_main_window() {
    let runtime = runtime_commands(include_str!("../src/api/auth.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "auth");
    for command in [
        "check_reachable",
        "check_mojang_services",
        "network_scope",
        "set_mojang_auth_use_mirror",
    ] {
        assert!(runtime.contains(command));
        assert!(manifest.contains(command));
    }
    let settings = runtime_commands(include_str!("../src/api/settings.rs"));
    assert!(settings.contains("proxy_set"));
    assert!(
        manifest_commands(include_str!("../build.rs"), "settings")
            .contains("proxy_set")
    );
    let capabilities: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert!(
        capabilities["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| window == "main")
    );
    for permission in ["auth:default", "settings:default"] {
        assert!(
            capabilities["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == permission)
        );
    }
}

#[test]
fn external_server_import_commands_are_registered_for_main() {
    let runtime = runtime_commands(include_str!("../src/api/servers.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "servers");
    for command in [
        "servers_inspect_import",
        "servers_import_external",
        "servers_retry_external",
    ] {
        assert!(runtime.contains(command));
        assert!(manifest.contains(command));
    }
    let capabilities: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert!(
        capabilities["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "main")
    );
    assert!(
        capabilities["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "servers:default")
    );
}
