#[test]
fn network_diagnostics_commands_have_runtime_manifest_and_window_access() {
    let runtime = include_str!("../src/api/settings.rs");
    let manifest = include_str!("../build.rs");
    let start = manifest.find("\"settings\",").unwrap();
    let commands = &manifest
        [start..manifest[start..].find(".default_permission").unwrap() + start];
    for command in ["network_test", "network_test_cancel"] {
        let handlers = runtime
            .split("tauri::generate_handler![")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap();
        assert!(handlers.split(',').any(|entry| entry.trim() == command));
        assert!(commands.contains(&format!("\"{command}\"")));
    }
    assert!(
        include_str!("../capabilities/plugins.json")
            .contains("\"settings:default\"")
    );
    assert!(include_str!("../capabilities/plugins.json").contains("\"main\""));
    assert!(
        include_str!("../src/main.rs")
            .contains(".plugin(api::settings::init())")
    );
}
