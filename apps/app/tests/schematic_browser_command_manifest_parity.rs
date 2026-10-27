//! Guards that the schematic-browser plugin's runtime handlers, build-time
//! command manifest, window capability grant and plugin registration all agree.
//!
//! Tauri resolves `plugin:<name>|<command>` at runtime against a manifest
//! generated at build time. A command missing from any one of those layers fails
//! at runtime rather than at compile time, so the four are compared here.
//!
//! Unlike `instance_command_manifest_parity.rs`, the manifest check is
//! **bidirectional**: a name present in one list but absent from the other is a
//! failure in either direction. A one-way check misses a manifest entry that has
//! no handler behind it.

const PLUGIN: &str = "schematic-browser";

/// Every command the plugin exposes.
const COMMANDS: &[&str] = &[
    "schematic_browser_list",
    "schematic_browser_detail",
    "schematic_browser_download_plan",
    "schematic_browser_download_apply",
    "schematic_browser_cancel",
    "schematic_browser_capabilities",
    "schematic_browser_health",
];

fn manifest_commands(build_rs: &str) -> Vec<String> {
    let marker = format!("\"{PLUGIN}\",");
    let start = build_rs
        .find(&marker)
        .unwrap_or_else(|| panic!("plugin {PLUGIN} missing from build.rs"));
    let tail = &build_rs[start..];
    let end = tail
        .find(".default_permission")
        .unwrap_or_else(|| panic!("plugin {PLUGIN} has no default_permission"));
    let block = &tail[..end];

    let mut commands = Vec::new();
    let mut rest = block;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        let value = &after[..close];
        // The plugin name itself is quoted in the same block.
        if value != PLUGIN && !value.is_empty() {
            commands.push(value.to_string());
        }
        rest = &after[close + 1..];
    }
    commands.sort();
    commands.dedup();
    commands
}

fn runtime_handlers(runtime_rs: &str) -> Vec<String> {
    let handlers = runtime_rs
        .split("tauri::generate_handler![")
        .nth(1)
        .expect("runtime module has no generate_handler!")
        .split(']')
        .next()
        .expect("unterminated generate_handler!")
        .to_string();
    let mut commands: Vec<String> = handlers
        .split(',')
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    commands.sort();
    commands.dedup();
    commands
}

#[test]
fn runtime_and_manifest_lists_match_in_both_directions() {
    let runtime = include_str!("../src/api/schematic_browser.rs");
    let build = include_str!("../build.rs");

    let handlers = runtime_handlers(runtime);
    let manifest = manifest_commands(build);

    let missing_from_manifest: Vec<&String> =
        handlers.iter().filter(|c| !manifest.contains(c)).collect();
    assert!(
        missing_from_manifest.is_empty(),
        "handlers with no build.rs manifest entry: {missing_from_manifest:?}"
    );

    let missing_from_runtime: Vec<&String> =
        manifest.iter().filter(|c| !handlers.contains(c)).collect();
    assert!(
        missing_from_runtime.is_empty(),
        "manifest entries with no runtime handler: {missing_from_runtime:?}"
    );
}

#[test]
fn every_documented_command_is_registered() {
    let runtime = include_str!("../src/api/schematic_browser.rs");
    let handlers = runtime_handlers(runtime);
    for command in COMMANDS {
        assert!(
            handlers.iter().any(|entry| entry == command),
            "{command} is not in generate_handler!"
        );
    }
}

#[test]
fn window_has_the_plugin_capability() {
    let capabilities = include_str!("../capabilities/plugins.json");
    assert!(
        capabilities.contains(&format!("\"{PLUGIN}:default\"")),
        "capabilities/plugins.json is missing {PLUGIN}:default"
    );
    assert!(
        capabilities.contains("\"main\""),
        "the capability must target the main window"
    );
}

#[test]
fn plugin_is_registered_on_the_tauri_builder() {
    let main = include_str!("../src/main.rs");
    assert!(
        main.contains(".plugin(api::schematic_browser::init())"),
        "main.rs never registers the schematic-browser plugin"
    );
}

#[test]
fn plugin_identifier_is_kebab_case() {
    // Tauri compares `Builder::new()` with the invoke name character for
    // character, so an underscore here fails only at runtime.
    let runtime = include_str!("../src/api/schematic_browser.rs");
    assert!(
        runtime.contains(&format!("Builder::new(\"{PLUGIN}\")")),
        "plugin identifier must be the kebab-case {PLUGIN}"
    );
    assert!(
        !runtime.contains("Builder::new(\"schematic_browser\")"),
        "underscored plugin identifiers break invoke at runtime"
    );
}
