//! Isolated import, installation, and failure-path regression tests.
use super::detect::tests::{forge, vanilla};
use super::launch::{imported_launch_args, validate_args, validate_jar_paths};
use super::*;
fn source() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("launch.jar"), vanilla()).unwrap();
    dir
}
#[test]
fn configuration_cannot_override_detected_identity_or_conflicts() {
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("config.txt"), b"resource").unwrap();
    let unresolved = inspect_configurable_server_import(src.path())
        .unwrap()
        .unwrap();
    let configuration = || ServerImportConfiguration {
        server_type: "forge".into(),
        game_version: "1.12.2".into(),
        loader_version: Some("14.23.5.2864".into()),
    };
    assert!(apply_configuration(unresolved.clone(), None).is_err());
    let resolved =
        apply_configuration(unresolved.clone(), Some(configuration())).unwrap();
    assert_eq!(resolved.game_version, "1.12.2");
    assert_eq!(resolved.problem, None);
    assert_eq!(unresolved.game_version, "");
    let mut conflicting = unresolved.clone();
    conflicting.problem = Some("conflicting-versions".into());
    assert!(apply_configuration(conflicting, Some(configuration())).is_err());
    let recognized = inspect_server_import(source().path()).unwrap().unwrap();
    assert!(apply_configuration(recognized, Some(configuration())).is_err());
    for invalid in ["../1.12.2", "1.12.2?download=evil", ""] {
        let mut configuration = configuration();
        configuration.game_version = invalid.into();
        assert!(
            apply_configuration(unresolved.clone(), Some(configuration))
                .is_err()
        );
    }
    let mut unsupported = configuration();
    unsupported.server_type = "neoforge".into();
    assert!(apply_configuration(unresolved, Some(unsupported)).is_err());
}
#[test]
fn configured_resource_copy_preserves_files_and_legacy_jar_validation_requires_dependencies()
 {
    let src = tempfile::tempdir().unwrap();
    let staging = tempfile::tempdir().unwrap();
    std::fs::create_dir(src.path().join("config")).unwrap();
    std::fs::write(src.path().join("config/test.cfg"), b"config").unwrap();
    let preview = inspect_configurable_server_import(src.path())
        .unwrap()
        .unwrap();
    let content = staging.path().join("content");
    prepare_source(src.path(), &content, &preview).unwrap();
    assert_eq!(
        std::fs::read(content.join("config/test.cfg")).unwrap(),
        b"config"
    );
    let jar = content.join("legacy.jar");
    std::fs::write(&jar, super::detect::tests::legacy_forge()).unwrap();
    assert!(validate_jar_paths(&content, &jar).is_err());
    std::fs::write(
        content.join("minecraft_server.1.12.2.jar"),
        b"server dependency",
    )
    .unwrap();
    std::fs::create_dir(content.join("libraries")).unwrap();
    std::fs::write(content.join("libraries/launchwrapper.jar"), b"library")
        .unwrap();
    assert!(validate_jar_paths(&content, &jar).is_ok());
}
#[cfg(not(feature = "tauri"))]
#[tokio::test]
#[ignore = "Requires network, AXOLOTL_TEST_JAVA=Java8 and a fresh temporary THESEUS_CONFIG_DIR"]
async fn manual_legacy_forge_install_and_explicit_start_are_isolated() {
    let config = std::fs::canonicalize(
        std::env::var_os("THESEUS_CONFIG_DIR")
            .expect("Use a new temporary data directory"),
    )
    .unwrap();
    let temporary = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(
        config.starts_with(temporary) || config.starts_with("/private/tmp")
    );
    assert!(
        ["release", "beta"]
            .iter()
            .all(|channel| !config.join(channel).join("app.db").exists())
    );
    let java = std::env::var("AXOLOTL_TEST_JAVA")
        .expect("Select an isolated-test Java8 executable");
    crate::event::EventState::init().await.unwrap();
    let state = State::init_for_test("red.ghs.axolotl.import-test".into())
        .await
        .unwrap();
    assert!(state.directories.settings_dir.starts_with(&config));
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("server.properties"), b"server-port=0\nserver-ip=127.0.0.1\nlevel-name=fixture-world\nonline-mode=false\n").unwrap();
    std::fs::write(src.path().join("eula.txt"), b"eula=false\n").unwrap();
    std::fs::write(src.path().join("start.sh"), b"exit 88\n").unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    let imported = import_external_server(
        src.path().to_string_lossy().into_owned(),
        preview,
        "Legacy Forge isolated test".into(),
        java,
        1024,
        Some(ServerImportConfiguration {
            server_type: "forge".into(),
            game_version: "1.12.2".into(),
            loader_version: Some("14.23.5.2864".into()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(imported.install_state, None, "{:?}", imported.install_error);
    let dest = server_path(&imported.id).await.unwrap();
    assert!(imported.jar_name.is_some());
    assert_eq!(imported_launch_args(&dest, &imported).unwrap()[0], "-jar");
    assert_eq!(
        std::fs::read(dest.join("eula.txt")).unwrap(),
        b"eula=false\n"
    );
    assert!(!dest.join("fixture-world").exists());
    assert!(
        !super::super::manage::get(&imported.id)
            .await
            .unwrap()
            .running
    );
    // Explicit test-only start; import itself kept the owner's EULA status and stayed stopped.
    if std::env::var("AXOLOTL_TEST_START").as_deref() == Ok("1") {
        std::fs::write(dest.join("eula.txt"), b"eula=true\n").unwrap();
        let mut command = crate::util::java_environment::tokio_command(
            imported.java_path.as_ref().unwrap(),
        );
        command
            .arg("-Xmx1G")
            .args(imported_launch_args(&dest, &imported).unwrap())
            .current_dir(&dest)
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
        let errors = tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Some(line) = lines.next_line().await.unwrap() {
                eprintln!("{line}");
            }
        });
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        let mut input = stdin;
        tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let mut done = false;
            while let Some(line) = lines.next_line().await.unwrap() {
                eprintln!("{line}");
                if line.contains("Done (") {
                    done = true;
                    input.write_all(b"stop\n").await.unwrap();
                }
            }
            let status = child.wait().await.unwrap();
            assert!(done, "Server never reached Done");
            assert!(status.success());
        })
        .await
        .expect("Fresh legacy Forge server startup timed out");
        errors.await.unwrap();
        assert!(dest.join("fixture-world").exists());
    }
    assert_eq!(
        std::fs::read(src.path().join("eula.txt")).unwrap(),
        b"eula=false\n"
    );
}
#[test]
fn nested_server_archive_import_uses_the_confirmed_payload_only() {
    use super::detect::tests::{spc, zip_files};
    let src = tempfile::tempdir().unwrap();
    let staging = tempfile::tempdir().unwrap();
    let inner = zip_files(&[
        ("server/manifest.json", spc().as_bytes()),
        ("server/config/test.txt", b"config"),
        ("server/mods/test.jar", b"mod"),
        ("server/world/level.dat", b"test-world"),
        ("server/eula.txt", b"eula=false\n"),
    ]);
    let path = src.path().join("outer.zip");
    std::fs::write(
        &path,
        zip_files(&[("payload.zip", &inner), ("readme.txt", b"outer only")]),
    )
    .unwrap();
    let preview = inspect_server_import(&path).unwrap().unwrap();
    let content = staging.path().join("content");
    prepare_source(&path, &content, &preview).unwrap();
    assert_eq!(
        std::fs::read(content.join("world/level.dat")).unwrap(),
        b"test-world"
    );
    assert_eq!(
        std::fs::read(content.join("eula.txt")).unwrap(),
        b"eula=false\n"
    );
    assert!(!content.join("payload.zip").exists());
    assert!(!content.join("readme.txt").exists());
    assert!(path.exists());
    let unsafe_inner = zip_files(&[
        ("manifest.json", spc().as_bytes()),
        ("../escape.txt", b"bad"),
    ]);
    std::fs::write(&path, zip_files(&[("payload.zip", &unsafe_inner)]))
        .unwrap();
    let preview = inspect_server_import(&path).unwrap().unwrap();
    assert!(
        prepare_source(&path, &staging.path().join("unsafe"), &preview)
            .is_err()
    );
    assert!(!staging.path().join("escape.txt").exists());
}
#[cfg(not(feature = "tauri"))]
#[tokio::test]
#[ignore = "Requires network and a fresh temporary THESEUS_CONFIG_DIR"]
async fn fabric_and_manual_vanilla_acquire_verified_launchers_without_starting()
{
    let config = std::fs::canonicalize(
        std::env::var_os("THESEUS_CONFIG_DIR")
            .expect("Use a new temporary data directory"),
    )
    .unwrap();
    let temporary = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(
        config.starts_with(temporary) || config.starts_with("/private/tmp")
    );
    assert!(
        ["release", "beta"]
            .iter()
            .all(|channel| !config.join(channel).join("app.db").exists())
    );
    crate::event::EventState::init().await.unwrap();
    let state = State::init_for_test("red.ghs.axolotl.import-test".into())
        .await
        .unwrap();
    assert!(state.directories.settings_dir.starts_with(&config));
    let source = tempfile::tempdir().unwrap();
    std::fs::write(
        source.path().join("manifest.json"),
        super::detect::tests::spc(),
    )
    .unwrap();
    std::fs::write(source.path().join("start.sh"), b"exit 88\n").unwrap();
    std::fs::write(source.path().join("eula.txt"), b"eula=false\n").unwrap();
    let preview = inspect_server_import(source.path()).unwrap().unwrap();
    let imported = import_external_server(
        source.path().to_string_lossy().into_owned(),
        preview,
        "Fabric declaration test".into(),
        "/fixture/not-executed-java".into(),
        2048,
        None,
    )
    .await
    .unwrap();
    assert_eq!(imported.install_state, None, "{:?}", imported.install_error);
    let dest = server_path(&imported.id).await.unwrap();
    assert!(dest.join(imported.jar_name.as_ref().unwrap()).is_file());
    assert_eq!(
        std::fs::read(dest.join("eula.txt")).unwrap(),
        b"eula=false\n"
    );
    assert!(!dest.join("world").exists());
    assert!(
        !super::super::manage::get(&imported.id)
            .await
            .unwrap()
            .running
    );
    let preview = inspect_server_import(&dest).unwrap().unwrap();
    assert!(!preview.needs_install);
    assert_eq!(preview.problem, None);
    let resources = tempfile::tempdir().unwrap();
    std::fs::write(resources.path().join("eula.txt"), b"eula=false\n").unwrap();
    std::fs::write(
        resources.path().join("server.properties"),
        b"server-port=25579\nlevel-name=World\n",
    )
    .unwrap();
    let preview = inspect_configurable_server_import(resources.path())
        .unwrap()
        .unwrap();
    let vanilla = import_external_server(
        resources.path().to_string_lossy().into_owned(),
        preview,
        "Manual Vanilla verification".into(),
        "/fixture/not-executed-java".into(),
        2048,
        Some(ServerImportConfiguration {
            server_type: "vanilla".into(),
            game_version: "1.20.1".into(),
            loader_version: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(vanilla.install_state, None, "{:?}", vanilla.install_error);
    let vanilla_dir = server_path(&vanilla.id).await.unwrap();
    assert_eq!(
        imported_launch_args(&vanilla_dir, &vanilla).unwrap()[0],
        "-jar"
    );
    assert_eq!(
        std::fs::read(vanilla_dir.join("eula.txt")).unwrap(),
        b"eula=false\n"
    );
    assert!(!vanilla_dir.join("World").exists());
    assert!(
        !super::super::manage::get(&vanilla.id)
            .await
            .unwrap()
            .running
    );
}
#[test]
fn external_server_copy_preserves_world_and_settings_without_source_manifest() {
    let src = source();
    let stage = tempfile::tempdir().unwrap();
    std::fs::create_dir(src.path().join("world")).unwrap();
    std::fs::write(src.path().join("world/level.dat"), b"saved-world").unwrap();
    std::fs::write(
        src.path().join("server.properties"),
        b"server-port=25570\nonline-mode=false\n",
    )
    .unwrap();
    std::fs::write(
        src.path().join("eula.txt"),
        b"# accepted by owner\neula=true\n",
    )
    .unwrap();
    std::fs::write(src.path().join("axolotl-server.json"), br#"{"preLaunchHook":"touch /tmp/no","id":"original","javaPath":"/private/java"}"#).unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    let dest = stage.path().join("content");
    prepare_source(src.path(), &dest, &preview).unwrap();
    for file in [
        "world/level.dat",
        "server.properties",
        "eula.txt",
        "launch.jar",
    ] {
        assert_eq!(
            std::fs::read(src.path().join(file)).unwrap(),
            std::fs::read(dest.join(file)).unwrap()
        );
    }
    assert!(!dest.join("axolotl-server.json").exists());
    assert!(src.path().join("axolotl-server.json").exists());
}
#[test]
fn external_server_copy_rejects_recursive_destination() {
    let src = source();
    let stage = tempfile::tempdir_in(src.path()).unwrap();
    let mut preview = inspect_server_import(src.path()).unwrap().unwrap();
    preview.inner_base.clear();
    assert!(
        prepare_source(src.path(), &stage.path().join("content"), &preview)
            .unwrap_err()
            .contains("outside")
    );
    assert!(src.path().join("launch.jar").exists());
}
#[test]
fn external_server_copy_detects_changed_metadata() {
    let src = source();
    let stage = tempfile::tempdir().unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    std::fs::write(src.path().join("launch.jar"), forge()).unwrap();
    assert!(
        prepare_source(src.path(), &stage.path().join("content"), &preview)
            .is_err()
    );
}
#[test]
fn external_server_zip_rejects_traversal_and_links() {
    for link in [false, true] {
        let src = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        let path = src.path().join("server.zip");
        let mut out =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        out.start_file("launch.jar", opts).unwrap();
        out.write_all(&vanilla()).unwrap();
        if link {
            out.add_symlink("world-link", "/private/user-world", opts)
                .unwrap();
        } else {
            out.start_file("../escape", opts).unwrap();
            out.write_all(b"bad").unwrap();
        }
        out.finish().unwrap();
        let preview = inspect_server_import(&path).unwrap().unwrap();
        assert!(
            prepare_source(&path, &stage.path().join("content"), &preview)
                .is_err()
        );
        assert!(!stage.path().join("escape").exists());
    }
}
#[cfg(unix)]
#[test]
fn external_server_folder_links_are_never_followed() {
    let src = source();
    let stage = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    std::fs::write(other.path().join("world"), b"private").unwrap();
    std::os::unix::fs::symlink(other.path(), src.path().join("world")).unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    assert!(
        prepare_source(src.path(), &stage.path().join("content"), &preview)
            .is_err()
    );
    assert_eq!(
        std::fs::read(other.path().join("world")).unwrap(),
        b"private"
    );
}
#[test]
fn external_server_launch_rejects_hooks_and_missing_dependencies() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("libraries")).unwrap();
    std::fs::write(root.path().join("libraries/ok.jar"), b"library").unwrap();
    let baseline = "--module-path libraries/ok.jar cpw.mods.bootstraplauncher.BootstrapLauncher --add-opens java.base/java.util.jar=cpw.mods.securejarhandler --launchTarget forge_server";
    assert!(validate_args(root.path(), baseline).is_ok());
    for args in [
        "@/tmp/user.args",
        "-javaagent:evil.jar",
        "-XX:OnError=exec",
        "-Djava.system.class.loader=Evil",
        "--class-path /private/lib.jar",
        "--class-path libraries/missing.jar",
    ] {
        assert!(
            validate_args(root.path(), &format!("{baseline} {args}")).is_err(),
            "{args}"
        );
    }
}
#[cfg(not(feature = "tauri"))]
#[tokio::test]
#[ignore = "Run explicitly with a fresh temporary THESEUS_CONFIG_DIR"]
async fn external_server_managed_import_and_repair_are_isolated() {
    // Refuse to initialize State unless the caller explicitly supplies a clean temporary data directory.
    let config = std::env::var_os("THESEUS_CONFIG_DIR")
        .expect("Run this test with a fresh temporary THESEUS_CONFIG_DIR");
    let config = std::fs::canonicalize(config)
        .expect("Create the isolated data directory before testing");
    let system_temp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(
        config.starts_with(system_temp) || config.starts_with("/private/tmp")
    );
    assert!(
        ["release", "beta"]
            .iter()
            .all(|channel| !config.join(channel).join("app.db").exists()),
        "This test requires a clean database"
    );
    crate::event::EventState::init().await.unwrap();
    let state = State::init_for_test("red.ghs.axolotl.import-test".into())
        .await
        .unwrap();
    assert!(state.directories.settings_dir.starts_with(&config));
    let src = source();
    std::fs::write(src.path().join("eula.txt"), b"eula=false\n").unwrap();
    std::fs::write(src.path().join("icon.png"), b"fixture-icon").unwrap();
    std::fs::write(
        src.path().join("server.properties"),
        b"server-port=25577\n",
    )
    .unwrap();
    std::fs::write(src.path().join("axolotl-server.json"), br#"{"id":"private","preLaunchHook":"never-run","javaPath":"/private/java"}"#).unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    let imported = import_external_server(
        src.path().to_string_lossy().into_owned(),
        preview.clone(),
        "External".into(),
        "/fixture/java17".into(),
        2048,
        None,
    )
    .await
    .unwrap();
    assert!(imported.external_import);
    assert_eq!(imported.install_state, None);
    assert_eq!(imported.pre_launch_hook, None);
    assert_eq!(imported.java_path.as_deref(), Some("/fixture/java17"));
    assert_ne!(imported.id, "private");
    let info = super::super::manage::get(&imported.id).await.unwrap();
    assert!(!info.running);
    assert!(!info.eula_accepted);
    assert_eq!(info.port, Some(25577));
    assert!(
        Path::new(imported.icon_path.as_ref().unwrap()).starts_with(&info.path)
    );
    assert!(src.path().join("axolotl-server.json").exists());
    let second = import_external_server(
        src.path().to_string_lossy().into_owned(),
        preview,
        "External".into(),
        "/fixture/java17".into(),
        2048,
        None,
    )
    .await
    .unwrap();
    assert_ne!(imported.id, second.id);
    // A missing dependency stays failed and cannot launch; restoring it allows revalidation without replacing the jar.
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    out.start_file("META-INF/MANIFEST.MF", opts).unwrap();
    out.write_all(b"Manifest-Version: 1.0\nMain-Class: net.minecraft.bundler.Main\nClass-Path: libraries/missing.jar\n").unwrap();
    out.start_file("version.json", opts).unwrap();
    out.write_all(br#"{"id":"1.20.1"}"#).unwrap();
    std::fs::write(
        src.path().join("launch.jar"),
        out.finish().unwrap().into_inner(),
    )
    .unwrap();
    let preview = inspect_server_import(src.path()).unwrap().unwrap();
    let failed = import_external_server(
        src.path().to_string_lossy().into_owned(),
        preview,
        "Broken".into(),
        "/fixture/no-java".into(),
        2048,
        None,
    )
    .await
    .unwrap();
    assert_eq!(failed.install_state, Some(InstallState::Failed));
    assert!(
        failed
            .install_error
            .as_deref()
            .unwrap()
            .contains("dependency")
    );
    assert!(
        super::super::lifecycle::start(&failed.id, None, None, None)
            .await
            .is_err()
    );
    let dest = server_path(&failed.id).await.unwrap();
    std::fs::create_dir(dest.join("libraries")).unwrap();
    std::fs::write(dest.join("libraries/missing.jar"), b"restored").unwrap();
    let repaired = retry_external_server(&failed.id).await.unwrap();
    assert_eq!(repaired.install_state, None);
    assert_eq!(repaired.install_error, None);
    assert!(!super::super::manage::get(&failed.id).await.unwrap().running);
}
#[cfg(not(feature = "tauri"))]
#[tokio::test]
#[ignore = "Requires network, AXOLOTL_TEST_JAVA and a fresh temporary THESEUS_CONFIG_DIR"]
async fn external_server_official_forge_installs_without_starting() {
    let config = std::fs::canonicalize(
        std::env::var_os("THESEUS_CONFIG_DIR")
            .expect("Use a new temporary data directory"),
    )
    .unwrap();
    let temporary = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(
        config.starts_with(temporary) || config.starts_with("/private/tmp")
    );
    assert!(
        ["release", "beta"]
            .iter()
            .all(|channel| !config.join(channel).join("app.db").exists())
    );
    let java = std::env::var("AXOLOTL_TEST_JAVA")
        .expect("Select an isolated-test Java 17 executable");
    crate::event::EventState::init().await.unwrap();
    let state = State::init_for_test("red.ghs.axolotl.import-test".into())
        .await
        .unwrap();
    assert!(state.directories.settings_dir.starts_with(&config));
    let source = tempfile::tempdir().unwrap();
    // The source is a metadata-only fixture. Only the official installer downloaded by Axolotl is executed.
    std::fs::write(source.path().join("forge.jar"), forge()).unwrap();
    std::fs::write(
        source.path().join("eula.txt"),
        b"# owner status\neula=false\n",
    )
    .unwrap();
    std::fs::write(source.path().join("start.sh"), b"exit 88\n").unwrap();
    let preview = inspect_server_import(source.path()).unwrap().unwrap();
    let began = std::time::Instant::now();
    let result = import_external_server(
        source.path().to_string_lossy().into_owned(),
        preview,
        "Forge install verification".into(),
        java,
        2048,
        None,
    )
    .await
    .unwrap();
    eprintln!("Official Forge import took {:?}", began.elapsed());
    assert_eq!(result.install_state, None, "{:?}", result.install_error);
    assert_eq!(result.game_version, "1.20.1");
    assert_eq!(result.loader_version.as_deref(), Some("47.4.16"));
    let dest = server_path(&result.id).await.unwrap();
    assert!(
        dest.join(
            "libraries/net/minecraftforge/forge/1.20.1-47.4.16/unix_args.txt"
        )
        .exists()
    );
    assert_eq!(
        std::fs::read(dest.join("eula.txt")).unwrap(),
        b"# owner status\neula=false\n"
    );
    assert!(!dest.join("world").exists());
    assert!(!super::super::manage::get(&result.id).await.unwrap().running);
    let installed_preview = inspect_server_import(&dest).unwrap().unwrap();
    assert!(!installed_preview.needs_install);
    assert_eq!(installed_preview.problem, None);
    let copied = import_external_server(
        dest.to_string_lossy().into_owned(),
        installed_preview,
        "Installed Forge verification".into(),
        result.java_path.clone().unwrap(),
        2048,
        None,
    )
    .await
    .unwrap();
    assert_ne!(copied.id, result.id);
    assert_eq!(copied.install_state, None, "{:?}", copied.install_error);
    let copied_dest = server_path(&copied.id).await.unwrap();
    assert!(dest.join("axolotl-server.json").exists());
    assert_eq!(
        std::fs::read(copied_dest.join("eula.txt")).unwrap(),
        b"# owner status\neula=false\n"
    );
    assert!(!copied_dest.join("world").exists());
    assert!(!super::super::manage::get(&copied.id).await.unwrap().running);
}

fn adapter_manifest() -> ServerManifest {
    serde_json::from_value(serde_json::json!({
        "id": "adapter-fixture",
        "name": "Adapter fixture",
        "serverType": "vanilla",
        "gameVersion": "1.20.1",
        "createdAt": "2026-10-23T00:00:00Z"
    }))
    .unwrap()
}

#[test]
fn lifecycle_adapter_leaves_existing_servers_to_the_original_launch_flow() {
    let root = tempfile::tempdir().unwrap();
    let mut manifest = adapter_manifest();
    manifest.install_state = Some(InstallState::Incomplete);
    assert!(
        launch::launch_args(root.path(), &manifest)
            .unwrap()
            .is_none()
    );
    manifest.external_import = true;
    assert!(launch::launch_args(root.path(), &manifest).is_err());
    manifest.install_state = None;
    manifest.jar_name = Some("server.jar".into());
    std::fs::write(root.path().join("server.jar"), vanilla()).unwrap();
    assert_eq!(
        launch::launch_args(root.path(), &manifest)
            .unwrap()
            .unwrap(),
        ["-jar", "server.jar", "nogui"]
    );
}

#[tokio::test]
async fn fabric_preparation_retains_installed_imports_and_validates_unattended_launchers()
 {
    let root = tempfile::tempdir().unwrap();
    let mut manifest = adapter_manifest();
    manifest.server_type = "fabric".into();
    manifest.jar_name = Some("server.jar".into());
    let cancellation = tokio_util::sync::CancellationToken::new();
    let jar = root.path().join("server.jar");
    std::fs::write(
        &jar,
        super::detect::tests::jar(
            "net.fabricmc.loader.impl.launch.server.FabricServerLauncher",
            &[],
        ),
    )
    .unwrap();
    manifest.external_import = true;
    prepare_fabric_launch("fixture", root.path(), &manifest, &cancellation)
        .await
        .unwrap();
    manifest.external_import = false;
    assert!(
        prepare_fabric_launch("fixture", root.path(), &manifest, &cancellation)
            .await
            .is_err()
    );
    manifest.external_import = true;
    std::fs::write(
        &jar,
        super::detect::tests::jar(
            "net.fabricmc.installer.ServerLauncher",
            &[("install.properties", "invalid-properties")],
        ),
    )
    .unwrap();
    assert!(
        prepare_fabric_launch("fixture", root.path(), &manifest, &cancellation)
            .await
            .is_err()
    );
}
