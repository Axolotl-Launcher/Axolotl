use std::collections::BTreeSet;

#[test]
fn mcp_commands_are_registered_and_limited_to_the_main_window() {
    let runtime = runtime_commands(include_str!("../src/api/mcp/mod.rs"))
        .into_iter()
        .map(|command| command.strip_prefix("commands::").unwrap_or(command))
        .collect::<BTreeSet<_>>();
    let manifest = manifest_commands(include_str!("../build.rs"), "mcp");
    assert_eq!(runtime, manifest);
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert!(
        capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "mcp:default")
    );
}

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
fn resource_analysis_commands_are_registered_and_allowed_for_the_main_window() {
    let runtime = runtime_commands(include_str!("../src/api/resources.rs"));
    let manifest =
        manifest_commands(include_str!("../build.rs"), "axolotl-resources");
    for command in [
        "resource_analysis_open",
        "resource_analysis_update",
        "resource_analysis_close",
        "resource_prepare_operation",
        "resource_start_content_refresh",
        "resource_get_content_refresh_state",
    ] {
        assert!(runtime.contains(command));
        assert!(manifest.contains(command));
    }
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
            .any(|permission| permission == "axolotl-resources:default")
    );
}

#[test]
fn content_refresh_commands_match_callers_and_events_are_limited_to_the_main_window()
 {
    let runtime = runtime_commands(include_str!("../src/api/resources.rs"));
    let manifest =
        manifest_commands(include_str!("../build.rs"), "axolotl-resources");
    let frontend =
        include_str!("../../app-frontend/src/helpers/content-refresh.ts");
    for command in [
        "resource_start_content_refresh",
        "resource_get_content_refresh_state",
    ] {
        assert!(runtime.contains(command));
        assert!(manifest.contains(command));
        assert!(
            frontend.contains(&format!("plugin:axolotl-resources|{command}"))
        );
    }
    let source = include_str!("../src/api/resources.rs");
    assert!(source.contains("get_webview_window(\"main\")"));
    assert!(source.contains("window.emit(\"resource_content_refresh\""));
    assert!(
        include_str!("../src/main.rs")
            .contains("initialize_content_refresh_events(&app).await?")
    );
    let capability: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert!(
        capability["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "axolotl-resources:default")
    );
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
        "instance_apply_content_update_plan",
        "instance_cancel_upgrade_planning",
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
    for command in [
        "servers_get_console_delta",
        "servers_send_console_input",
        "servers_resize_console",
        "servers_kill",
        "servers_update_settings",
        "servers_install_neoforge",
        "servers_install_modpack",
    ] {
        assert!(runtime.contains(command));
        assert!(manifest.contains(command));
    }
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
fn curseforge_runtime_handlers_match_the_build_manifest() {
    let runtime = runtime_commands(include_str!("../src/api/curseforge.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "curseforge");
    assert_eq!(
        runtime, manifest,
        "curseforge commands must be identical in the runtime handler and the \
         build-time manifest"
    );
}

#[test]
fn seed_map_runtime_handlers_match_the_build_manifest() {
    let runtime = runtime_commands(include_str!("../src/api/seed_map.rs"));
    let manifest = manifest_commands(include_str!("../build.rs"), "seed-map");
    assert_eq!(
        runtime, manifest,
        "seed map commands must be identical in the runtime handler and the \
         build-time manifest"
    );
    assert!(runtime.contains("seed_map_cancel_ore_scan"));
}

#[test]
fn mod_translation_runtime_handlers_match_the_build_manifest() {
    let runtime =
        runtime_commands(include_str!("../src/api/mod_translation.rs"));
    let manifest =
        manifest_commands(include_str!("../build.rs"), "mod-translation");
    assert_eq!(
        runtime, manifest,
        "mod translation commands must be identical in the runtime handler \
         and the build-time manifest"
    );
    assert!(runtime.contains("mod_translation_cancel_analysis"));
    let capabilities: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/plugins.json"))
            .unwrap();
    assert!(
        capabilities["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|permission| permission == "mod-translation:default")
    );
}
