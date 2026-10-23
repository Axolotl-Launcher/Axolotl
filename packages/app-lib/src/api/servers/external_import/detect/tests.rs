//! Detection fixtures and regressions; no real server data is accessed.
use super::*;
use std::io::Write;
pub(crate) fn jar(main: &str, files: &[(&str, &str)]) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    out.start_file("META-INF/MANIFEST.MF", options).unwrap();
    write!(out, "Manifest-Version: 1.0\nMain-Class: {main}\n").unwrap();
    for (name, value) in files {
        out.start_file(*name, options).unwrap();
        out.write_all(value.as_bytes()).unwrap();
    }
    out.finish().unwrap().into_inner()
}
pub(crate) fn forge() -> Vec<u8> {
    jar(
        "net.minecraftforge.installer.SimpleInstaller",
        &[
            (
                "install_profile.json",
                r#"{"spec":1,"minecraft":"1.20.1","version":"1.20.1-forge-47.4.16","json":"/version.json","libraries":[{"name":"net.minecraftforge:forge:1.20.1-47.4.16:universal"}]}"#,
            ),
            (
                "version.json",
                r#"{"id":"1.20.1-forge-47.4.16","inheritsFrom":"1.20.1"}"#,
            ),
        ],
    )
}
pub(crate) fn vanilla() -> Vec<u8> {
    jar(
        "net.minecraft.bundler.Main",
        &[("version.json", r#"{"id":"1.20.1"}"#)],
    )
}
#[test]
fn server_manifest_unfolding_preserves_classpath_segments() {
    assert_eq!(
        manifest_attribute(
            "Class-Path: libraries/net/minecraftforge/\n forge.jar\n",
            "class-path"
        )
        .as_deref(),
        Some("libraries/net/minecraftforge/forge.jar")
    );
}
pub(crate) fn legacy_forge() -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    out.start_file(
        "META-INF/MANIFEST.MF",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    out.write_all(b"Manifest-Version: 1.0\nMain-Class: net.minecraftforge.fml.relauncher.ServerLaunchWrapper\nClass-Path: minecraft_server.1.12.2.jar libraries/launchwrapper.jar\n\nName: net/minecraftforge/common/\nImplementation-Version: 14.23.5.2864\n").unwrap();
    out.finish().unwrap().into_inner()
}
#[test]
fn resource_only_server_settings_require_configuration_without_guessing_versions()
 {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("mods")).unwrap();
    std::fs::write(root.path().join("mods/example.jar"), b"mod").unwrap();
    std::fs::write(
        root.path().join("server.properties"),
        b"server-port=25577\nlevel-name=World\n",
    )
    .unwrap();
    let preview = inspect_server_import(root.path()).unwrap().unwrap();
    assert_eq!(preview.problem.as_deref(), Some("configuration-required"));
    assert_eq!(preview.server_type, "unknown");
    assert_eq!(preview.game_version, "");
    assert_eq!(preview.loader_version, None);
    assert!(matches!(
        crate::api::drop_classifier::classify_dropped_item(root.path()),
        crate::api::drop_classifier::DroppedItemType::Server { .. }
    ));
}
#[test]
fn explicit_server_action_keeps_resource_directories_at_the_package_root() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("wrapper/config")).unwrap();
    std::fs::write(root.path().join("wrapper/config/options.cfg"), b"config")
        .unwrap();
    assert!(inspect_server_import(root.path()).unwrap().is_none());
    let preview = inspect_configurable_server_import(root.path())
        .unwrap()
        .unwrap();
    assert_eq!(preview.inner_base, "wrapper");
    assert_eq!(preview.problem.as_deref(), Some("configuration-required"));
    std::fs::remove_dir_all(root.path().join("wrapper")).unwrap();
    std::fs::create_dir(root.path().join("mods")).unwrap();
    std::fs::write(root.path().join("mods/installer.jar"), forge()).unwrap();
    assert!(inspect_server_import(root.path()).unwrap().is_none());
    std::fs::remove_dir_all(root.path().join("mods")).unwrap();
    std::fs::create_dir_all(root.path().join("versions/1.12.2")).unwrap();
    std::fs::write(root.path().join("versions/1.12.2/1.12.2.json"), b"{}")
        .unwrap();
    assert_eq!(
        inspect_configurable_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("client-server-conflict")
    );
}
#[test]
fn legacy_forge_identity_comes_from_server_entry_and_manifest_dependencies() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("renamed.jar"), legacy_forge()).unwrap();
    let preview = inspect_server_import(root.path()).unwrap().unwrap();
    assert_eq!(preview.server_type, "forge");
    assert_eq!(preview.game_version, "1.12.2");
    assert_eq!(preview.loader_version.as_deref(), Some("14.23.5.2864"));
    assert_eq!(preview.jar_name.as_deref(), Some("renamed.jar"));
    assert!(!preview.needs_install);
    assert_eq!(preview.problem, None);
}
pub(crate) fn spc() -> &'static str {
    r#"{"serverPackCreatorVersion":"8.1.2","minecraftVersion":"1.20.1","modloader":"Fabric","modloaderVersion":"0.19.3","files":["mods","config"]}"#
}
pub(crate) fn zip_files(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        out.start_file(*name, options).unwrap();
        out.write_all(bytes).unwrap();
    }
    out.finish().unwrap().into_inner()
}
#[test]
fn declared_server_package_needs_no_launcher_or_scripts() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("manifest.json"), spc()).unwrap();
    let preview = inspect_server_import(root.path()).unwrap().unwrap();
    assert_eq!(preview.server_type, "fabric");
    assert_eq!(preview.game_version, "1.20.1");
    assert_eq!(preview.loader_version.as_deref(), Some("0.19.3"));
    assert_eq!(preview.problem, None);
    assert!(preview.needs_install);
    assert_eq!(preview.jar_name, None);
    assert!(matches!(
        crate::api::drop_classifier::classify_dropped_item(root.path()),
        crate::api::drop_classifier::DroppedItemType::Server { .. }
    ));
    std::fs::write(root.path().join("variables.txt"), "MINECRAFT_VERSION=1.20.1\nMODLOADER=Fabric\nMODLOADER_VERSION=0.19.2\nJAVA=$(touch /never-execute)\n").unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
}
#[test]
fn declarations_are_cross_checked_with_launchers_and_do_not_capture_clients() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("manifest.json"), r#"{"manifestType":"minecraftModpack","minecraft":{"version":"1.20.1","modLoaders":[{"id":"fabric-0.19.3"}]}}"#).unwrap();
    assert!(inspect_server_import(root.path()).unwrap().is_none());
    std::fs::write(root.path().join("variables.txt"), "MINECRAFT_VERSION=1.20.1\nMODLOADER=Fabric\nMODLOADER_VERSION=0.19.3\n").unwrap();
    assert!(inspect_server_import(root.path()).unwrap().is_none());
    std::fs::write(root.path().join("manifest.json"), spc()).unwrap();
    std::fs::write(
        root.path().join("renamed.jar"),
        jar(
            "net.fabricmc.installer.ServerLauncher",
            &[(
                "install.properties",
                "game-version=1.20.1\nfabric-loader-version=0.19.3\n",
            )],
        ),
    )
    .unwrap();
    let result = inspect_server_import(root.path()).unwrap().unwrap();
    assert_eq!(result.problem, None);
    assert!(!result.needs_install);
    assert_eq!(result.jar_name.as_deref(), Some("renamed.jar"));
    std::fs::write(root.path().join("renamed.jar"), vanilla()).unwrap();
    // A vanilla jar is a dependency; declaration still requires a Fabric launcher.
    assert!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .needs_install
    );
    std::fs::write(root.path().join("renamed.jar"), forge()).unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
}
#[test]
fn archive_wrappers_keep_container_paths_separate_from_server_identity() {
    let root = tempfile::tempdir().unwrap();
    let inner = zip_files(&[
        ("folder/manifest.json", spc().as_bytes()),
        ("folder/mods/a.jar", b"mod"),
    ]);
    let middle =
        zip_files(&[("wrapped.zip", &inner), ("readme.txt", b"documentation")]);
    let outer = zip_files(&[("wrapper/second.zip", &middle)]);
    let path = root.path().join("arbitrary.zip");
    std::fs::write(&path, outer).unwrap();
    let result = inspect_server_import(&path).unwrap().unwrap();
    assert_eq!(result.archives, ["wrapper/second.zip", "wrapped.zip"]);
    assert_eq!(result.inner_base, "folder");
    assert_eq!(result.server_type, "fabric");
    assert!(inspect_server_drop(&path, false).unwrap().is_none());
    assert!(matches!(
        crate::api::drop_classifier::classify_dropped_item_without_nested_unpack(&path),
        crate::api::drop_classifier::DroppedItemType::Unknown { reason }
            if reason.contains("nested archives")
    ));
    assert!(matches!(
        crate::api::drop_classifier::classify_dropped_item_with_candidates(
            &path, true
        ),
        crate::api::drop_classifier::DroppedItemType::Server { .. }
    ));
    let mut wrapped = inner;
    for _ in 0..4 {
        wrapped = zip_files(&[("nested.zip", &wrapped)]);
    }
    std::fs::write(&path, wrapped).unwrap();
    assert!(inspect_server_import(&path).unwrap().is_none());
}
#[test]
fn external_server_installers_precede_generic_modded_clients() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("arbitrary-name.jar"), forge()).unwrap();
    std::fs::create_dir(root.path().join("mods")).unwrap();
    std::fs::write(root.path().join("mods/example.jar"), b"mod").unwrap();
    assert!(
        matches!(crate::api::drop_classifier::classify_dropped_item_with_candidates(root.path(), false), crate::api::drop_classifier::DroppedItemType::Server { preview, .. } if preview.game_version == "1.20.1" && preview.loader_version.as_deref() == Some("47.4.16") && preview.needs_install)
    );
}
#[test]
fn external_server_client_mods_are_not_server_evidence() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("mods")).unwrap();
    std::fs::write(root.path().join("mods/server-mod.jar"), forge()).unwrap();
    std::fs::write(
        root.path().join("server.jar"),
        jar(
            "net.minecraft.client.main.Main",
            &[("version.json", r#"{"id":"1.20.1"}"#)],
        ),
    )
    .unwrap();
    assert!(inspect_server_import(root.path()).unwrap().is_none());
    assert!(!matches!(
        crate::api::drop_classifier::classify_dropped_item(root.path()),
        crate::api::drop_classifier::DroppedItemType::Server { .. }
    ));
}
#[test]
fn external_server_zip_wrapper_detection_preserves_root() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("backup.zip");
    let mut out = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    out.start_file("outer/inner/server/renamed.jar", opts)
        .unwrap();
    out.write_all(&forge()).unwrap();
    out.start_file("outer/inner/server/mods/a.jar", opts)
        .unwrap();
    out.write_all(b"mod").unwrap();
    out.start_file("__MACOSX/noise", opts).unwrap();
    out.write_all(b"noise").unwrap();
    out.finish().unwrap();
    let preview = inspect_server_import(&path).unwrap().unwrap();
    assert_eq!(preview.inner_base, "outer/inner/server");
    assert!(preview.needs_install);
}
#[test]
fn external_server_fabric_and_paper_and_vanilla_use_content() {
    for (kind, bytes) in [
        ("vanilla", vanilla()),
        (
            "paper",
            jar(
                "io.papermc.paperclip.Main",
                &[("version.json", r#"{"id":"1.20.1"}"#)],
            ),
        ),
        (
            "fabric",
            jar(
                "net.fabricmc.installer.ServerLauncher",
                &[(
                    "install.properties",
                    "game-version=1.20.1\nfabric-loader-version=0.16.14\n",
                )],
            ),
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("renamed.jar"), bytes).unwrap();
        let preview = inspect_server_import(root.path()).unwrap().unwrap();
        assert_eq!(preview.server_type, kind);
        assert_eq!(preview.game_version, "1.20.1");
        assert_eq!(preview.problem, None);
        assert_eq!(preview.jar_name.as_deref(), Some("renamed.jar"));
    }
}
#[test]
fn external_server_conflicts_and_missing_versions_are_blocked() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("first.jar"), vanilla()).unwrap();
    std::fs::write(root.path().join("second.jar"), vanilla()).unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("multiple-launchers")
    );
    std::fs::write(
        root.path().join("second.jar"),
        jar(
            "net.minecraft.bundler.Main",
            &[("version.json", r#"{"id":"1.21.4"}"#)],
        ),
    )
    .unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
    std::fs::remove_file(root.path().join("second.jar")).unwrap();
    std::fs::write(
        root.path().join("first.jar"),
        jar("net.minecraft.bundler.Main", &[]),
    )
    .unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("missing-version")
    );
}
#[test]
fn external_server_installed_forge_combines_installer_metadata() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("forge.jar"), forge()).unwrap();
    let libs = root
        .path()
        .join("libraries/net/minecraftforge/forge/1.20.1-47.4.16");
    std::fs::create_dir_all(&libs).unwrap();
    std::fs::write(libs.join("unix_args.txt"),"--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.4.16").unwrap();
    let result = inspect_server_import(root.path()).unwrap().unwrap();
    assert!(!result.needs_install);
    assert_eq!(result.problem, None);
    std::fs::write(libs.join("win_args.txt"),"--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.0.0").unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
}
#[test]
fn external_server_installed_conflict_survives_matching_installer() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("forge.jar"), forge()).unwrap();
    let libs = root
        .path()
        .join("libraries/net/minecraftforge/forge/1.20.1-47.4.16");
    std::fs::create_dir_all(&libs).unwrap();
    std::fs::write(libs.join("unix_args.txt"), "--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.0.0").unwrap();
    std::fs::write(libs.join("win_args.txt"), "--launchTarget forge_server --fml.mcVersion 1.20.1 --fml.forgeVersion 47.4.16").unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
}
#[test]
fn external_server_forge_metadata_is_cross_checked() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("installer.jar"), forge()).unwrap();
    let result = inspect_server_import(root.path()).unwrap().unwrap();
    assert_eq!(result.loader_version.as_deref(), Some("47.4.16"));
    assert_eq!(result.problem, None);
    std::fs::write(root.path().join("installer.jar"),jar("net.minecraftforge.installer.SimpleInstaller",&[
        ("install_profile.json",r#"{"minecraft":"1.20.1","version":"1.20.1-forge-47.4.16"}"#),
        ("version.json",r#"{"inheritsFrom":"1.20.1","id":"1.20.1-forge-47.0.0"}"#),
    ])).unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("conflicting-versions")
    );
    std::fs::write(root.path().join("installer.jar"),jar("net.minecraftforge.installer.SimpleInstaller",&[("install_profile.json",r#"{"minecraft":"1.20.1","path":"net.minecraftforge:forge:1.20.1-47.4.16:universal"}"#)])).unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .loader_version
            .as_deref(),
        Some("47.4.16")
    );
}
#[test]
fn external_server_bundler_cache_is_not_a_client_instance() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("server.jar"), vanilla()).unwrap();
    std::fs::create_dir_all(root.path().join("versions/1.20.1")).unwrap();
    std::fs::write(
        root.path().join("versions/1.20.1/server.jar"),
        b"bundled-server",
    )
    .unwrap();
    assert_eq!(
        inspect_server_import(root.path()).unwrap().unwrap().problem,
        None
    );
    std::fs::write(root.path().join("versions/1.20.1/1.20.1.json"), b"{}")
        .unwrap();
    assert_eq!(
        inspect_server_import(root.path())
            .unwrap()
            .unwrap()
            .problem
            .as_deref(),
        Some("client-server-conflict")
    );
}
