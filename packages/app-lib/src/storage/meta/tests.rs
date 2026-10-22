use super::*;
use serde_json::json;

fn directories(root: &Path) -> DirectoryInfo {
    DirectoryInfo {
        settings_dir: root.into(),
        config_dir: root.into(),
        app_identifier: "test".into(),
    }
}

async fn fixture(root: &Path, id: &str, hash: &str) {
    let version = json!({
        "id": id, "assetIndex": {"id": id, "sha1": "", "size": 0, "totalSize": 0, "url": ""},
        "assets": "legacy", "downloads": {}, "libraries": [{"name": "example:shared:1.0", "natives": {"windows": "natives-windows-${arch}"}}],
        "mainClass": "Main", "minimumLauncherVersion": 0,
        "releaseTime": "2020-01-01T00:00:00Z", "time": "2020-01-01T00:00:00Z", "type": "release"
    });
    let version_path = root.join("versions").join(id);
    fs::create_dir_all(&version_path).await.unwrap();
    fs::write(
        version_path.join(format!("{id}.json")),
        serde_json::to_vec(&version).unwrap(),
    )
    .await
    .unwrap();
    fs::create_dir_all(root.join("assets/indexes"))
        .await
        .unwrap();
    fs::write(
        root.join("assets/indexes").join(format!("{id}.json")),
        serde_json::to_vec(
            &json!({"objects": {"sound/test.ogg": {"hash": hash, "size": 3}}}),
        )
        .unwrap(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn shared_assets_and_legacy_paths_are_retained_across_versions() {
    let temp = tempfile::tempdir().unwrap();
    let hash = "a".repeat(40);
    fixture(temp.path(), "first", &hash).await;
    fixture(temp.path(), "second", &hash).await;
    let mut refs = References::default();
    version_references(temp.path(), "first", &mut refs)
        .await
        .unwrap();
    version_references(temp.path(), "second", &mut refs)
        .await
        .unwrap();
    assert_eq!(refs.used(&format!("assets/objects/aa/{hash}")), Some(true));
    assert_eq!(
        refs.used("libraries/example/shared/1.0/shared-1.0.jar"),
        Some(true)
    );
    assert_eq!(refs.used("resources/sound/test.ogg"), Some(true));
    assert_eq!(
        refs.used(&format!("assets/objects/bb/{}", "b".repeat(40))),
        Some(false)
    );
    assert_eq!(
        refs.used(
            "libraries/example/shared/1.0/shared-1.0-natives-windows-32.jar"
        ),
        Some(true)
    );
    assert_eq!(
        refs.used(
            "libraries/example/shared/1.0/shared-1.0-natives-windows-64.jar"
        ),
        Some(true)
    );
    refs.blocked = true;
    assert_eq!(refs.used("assets/objects/bb/orphan"), None);
}

#[tokio::test]
async fn malformed_asset_hash_and_path_escape_fail_reference_analysis() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path(), "test", "bad").await;
    assert!(
        version_references(temp.path(), "test", &mut References::default())
            .await
            .is_err()
    );
    for path in [
        "../outside",
        "libraries/../../outside",
        "C:/outside",
        "libraries\\outside",
    ] {
        assert!(References::default().keep(path.into()).is_err());
    }
}

#[tokio::test]
async fn cleanup_rechecks_references_and_file_changes() {
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let root_path = directories.metadata_dir();
    fs::create_dir_all(root_path.join("libraries/example/test/1.0"))
        .await
        .unwrap();
    for name in ["unused.jar", "changed.jar", "newly-used.jar"] {
        fs::write(
            root_path.join("libraries/example/test/1.0").join(name),
            [0; 3],
        )
        .await
        .unwrap();
    }
    let root = opened_root(&directories).unwrap();
    let mut groups = BTreeMap::new();
    let mut files = vec![];
    collect(
        &root,
        "",
        &References::default(),
        &mut groups,
        &mut files,
        &mut vec![],
    )
    .unwrap();
    assert_eq!(files.len(), 3);
    fs::write(
        root_path.join("libraries/example/test/1.0/changed.jar"),
        [0; 9],
    )
    .await
    .unwrap();
    let mut refs = References::default();
    refs.keep("libraries/example/test/1.0/newly-used.jar".into())
        .unwrap();
    let result = remove_candidates(
        &directories,
        files,
        &HashSet::from(["libraries".into()]),
        &refs,
    )
    .unwrap();
    assert_eq!(result.removed_files, 1);
    assert_eq!(result.removed_bytes, 3);
    assert_eq!(result.skipped_files, 2);
    assert!(result.failures.is_empty());
    assert!(
        !root_path
            .join("libraries/example/test/1.0/unused.jar")
            .exists()
    );
    assert!(
        root_path
            .join("libraries/example/test/1.0/changed.jar")
            .exists()
    );
    assert!(
        root_path
            .join("libraries/example/test/1.0/newly-used.jar")
            .exists()
    );
}

#[tokio::test]
async fn unconfirmed_categories_and_processor_libraries_are_never_candidates() {
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    for path in [
        "java_versions/java/bin/java.exe",
        "game-locales/v1/snapshots/a.json",
        "unknown/data",
        "libraries/example.jar",
    ] {
        let path = directories.metadata_dir().join(path);
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();
        fs::write(path, [0; 8]).await.unwrap();
    }
    let mut refs = References::default();
    refs.uncertain_categories.insert("libraries");
    let mut groups = BTreeMap::new();
    let mut files = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &refs,
        &mut groups,
        &mut files,
        &mut vec![],
    )
    .unwrap();
    assert!(files.is_empty());
    assert_eq!(groups.values().map(|group| group.unknown).sum::<u64>(), 32);
}

#[tokio::test]
async fn cleanup_cannot_follow_a_replaced_parent_directory() {
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let parent = directories
        .metadata_dir()
        .join("libraries/example/test/1.0");
    fs::create_dir_all(&parent).await.unwrap();
    fs::write(parent.join("unused.jar"), [0; 3]).await.unwrap();
    let mut files = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &References::default(),
        &mut BTreeMap::new(),
        &mut files,
        &mut vec![],
    )
    .unwrap();
    let original = temp.path().join("original");
    fs::rename(&parent, &original).await.unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside).await.unwrap();
    fs::write(outside.join("unused.jar"), [0; 3]).await.unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &parent).unwrap();
    #[cfg(windows)]
    junction::create(&outside, &parent).unwrap();
    let result = remove_candidates(
        &directories,
        files,
        &HashSet::from(["libraries".into()]),
        &References::default(),
    )
    .unwrap();
    assert_eq!(result.removed_files, 0);
    assert_eq!(result.failures.len(), 1);
    assert!(outside.join("unused.jar").exists());
    assert!(original.join("unused.jar").exists());
}

#[test]
fn unknown_files_inside_recognized_folders_are_kept() {
    let refs = References::default();
    for path in [
        "versions/old/user-notes.txt",
        "libraries/custom/file.txt",
        "assets/custom/data",
        "assets/objects/aa/not-a-hash",
        "log_configs/user.txt",
    ] {
        assert_eq!(refs.used(path), None, "{path}");
    }
    assert_eq!(refs.used("versions/old/old.jar"), Some(false));
}

#[tokio::test]
async fn channel_references_union_versions_and_detect_pending_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let mut refs = References::default();
    for version in ["first", "second"] {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO instances (id, path, install_stage, launcher_feature_version, name, created, modified) VALUES ('instance', 'example', 'installed', 'none', 'Example', 0, 0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_content_sets (id, instance_id, name, source_kind, status, game_version, loader, created, modified) VALUES ('set', 'instance', 'Default', 'local', 'available', ?, 'vanilla', 0, 0)").bind(version).execute(&pool).await.unwrap();
        channel_references(&pool, &mut refs, &directories)
            .await
            .unwrap();
        if version == "second" {
            sqlx::query("INSERT INTO install_jobs(id, kind, status, state, created, modified) VALUES ('job', 'install_instance', 'failed', '{\"rollback\":{}}', 0, 0)").execute(&pool).await.unwrap();
            let mut recovering = References::default();
            channel_references(&pool, &mut recovering, &directories)
                .await
                .unwrap();
            assert!(recovering.busy);
            assert_eq!(recovering.used("versions/orphan/orphan.jar"), None);
        }
        pool.close().await;
    }
    assert!(refs.versions.contains("first"));
    assert!(refs.versions.contains("second"));
    assert_eq!(refs.used("versions/first/first.jar"), Some(true));
    assert_eq!(refs.used("versions/second/second.jar"), Some(true));
    assert_eq!(refs.used("versions/orphan/orphan.jar"), Some(false));
}

#[tokio::test]
async fn shared_metadata_leases_exclude_cleanup_until_every_user_finishes() {
    use fs4::tokio::AsyncFileExt;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("lock");
    let open = || async {
        tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .unwrap()
    };
    let first = open().await;
    let second = open().await;
    let cleanup = open().await;
    first.lock_shared().unwrap();
    second.lock_shared().unwrap();
    assert!(!cleanup.try_lock_exclusive().unwrap());
    drop(first);
    assert!(!cleanup.try_lock_exclusive().unwrap());
    drop(second);
    assert!(cleanup.try_lock_exclusive().unwrap());
}

#[tokio::test]
async fn missing_version_preserves_known_usage_and_limits_unknown_dependencies()
{
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let root_path = directories.metadata_dir();
    let hash = "a".repeat(40);
    fixture(&root_path, "current", &hash).await;
    let mut refs = References::default();
    refs.manifests.extend(["current".into(), "missing".into()]);
    refs.versions.extend(["current".into(), "missing".into()]);
    refs.natives.extend(["current".into(), "missing".into()]);
    resolve_manifests(&root_path, &mut refs).await;
    assert!(!refs.blocked);
    assert_eq!(refs.warnings.len(), 1);
    assert!(refs.warnings[0].subject.contains("missing"));
    assert_eq!(refs.used("versions/current/current.jar"), Some(true));
    assert_eq!(
        refs.used("libraries/example/shared/1.0/shared-1.0.jar"),
        Some(true)
    );
    assert_eq!(refs.used(&format!("assets/objects/aa/{hash}")), Some(true));
    assert_eq!(refs.used("versions/old/old.jar"), Some(false));
    assert_eq!(refs.used("natives/old/native.dll"), Some(false));
    assert_eq!(
        refs.used("libraries/example/unknown/1.0/unknown-1.0.jar"),
        None
    );
    assert_eq!(refs.used("assets/indexes/unknown.json"), None);
    assert_eq!(refs.used("log_configs/unknown.xml"), None);

    for (path, bytes) in [
        ("versions/old/old.jar", 7),
        ("natives/current/native.dll", 11),
        ("libraries/example/unknown/1.0/unknown-1.0.jar", 13),
    ] {
        let path = root_path.join(path);
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();
        fs::write(path, vec![0; bytes]).await.unwrap();
    }
    let mut groups = BTreeMap::new();
    let mut files = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &refs,
        &mut groups,
        &mut files,
        &mut vec![],
    )
    .unwrap();
    assert!(groups.values().map(|group| group.used).sum::<u64>() >= 11);
    assert_eq!(groups.values().map(|group| group.unused).sum::<u64>(), 7);
    assert!(groups.values().map(|group| group.unknown).sum::<u64>() >= 13);
    let result = remove_candidates(
        &directories,
        files,
        &HashSet::from(["versions/old".into()]),
        &refs,
    )
    .unwrap();
    assert_eq!(result.removed_bytes, 7);
    assert!(root_path.join("natives/current/native.dll").exists());
    assert!(
        root_path
            .join("libraries/example/unknown/1.0/unknown-1.0.jar")
            .exists()
    );
}

#[test]
fn known_references_remain_visible_when_other_references_cannot_be_verified() {
    let mut refs = References::default();
    refs.versions.insert("current".into());
    refs.keep("libraries/example/shared/1.0/shared-1.0.jar".into())
        .unwrap();
    refs.uncertain_categories.insert("libraries");
    assert_eq!(
        refs.used("libraries/example/shared/1.0/shared-1.0.jar"),
        Some(true)
    );
    assert_eq!(refs.used("libraries/example/other/1.0/other.jar"), None);
    refs.blocked = true;
    assert_eq!(refs.used("versions/current/current.jar"), Some(true));
    assert_eq!(refs.used("versions/old/old.jar"), None);
    refs.blocked = false;
    refs.busy = true;
    assert_eq!(refs.used("versions/current/current.jar"), Some(true));
    assert_eq!(refs.used("versions/old/old.jar"), None);
}

#[tokio::test]
async fn cleanup_removes_version_format_markers_and_empty_folders() {
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let version = directories.metadata_dir().join("versions/old");
    fs::create_dir_all(&version).await.unwrap();
    for (name, bytes) in [
        ("old.jar", 12),
        ("old.json", 8),
        ("old.json.axolotl-format", 1),
    ] {
        fs::write(version.join(name), vec![0; bytes]).await.unwrap();
    }
    let mut groups = BTreeMap::new();
    let mut candidates = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &References::default(),
        &mut groups,
        &mut candidates,
        &mut vec![],
    )
    .unwrap();
    assert_eq!(groups["versions/old"].unused, 21);
    assert_eq!(groups["versions/old"].unknown, 0);
    let result = remove_candidates(
        &directories,
        candidates,
        &HashSet::from(["versions/old".into()]),
        &References::default(),
    )
    .unwrap();
    assert_eq!(result.removed_files, 3);
    assert_eq!(result.removed_groups["versions/old"].bytes, 21);
    assert!(result.failures.is_empty());
    assert!(!version.exists());
}

#[tokio::test]
async fn an_orphaned_format_marker_can_be_cleaned_without_touching_user_files()
{
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let version = directories.metadata_dir().join("versions/old");
    fs::create_dir_all(&version).await.unwrap();
    fs::write(version.join("old.json.axolotl-format"), "1")
        .await
        .unwrap();
    fs::write(version.join("user-notes.txt"), "keep")
        .await
        .unwrap();
    let mut candidates = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &References::default(),
        &mut BTreeMap::new(),
        &mut candidates,
        &mut vec![],
    )
    .unwrap();
    let result = remove_candidates(
        &directories,
        candidates,
        &HashSet::from(["versions/old".into()]),
        &References::default(),
    )
    .unwrap();
    assert_eq!(result.removed_bytes, 1);
    assert!(version.join("user-notes.txt").exists());
}

#[tokio::test]
async fn processor_inputs_outputs_and_classpaths_are_resolved_locally() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fixture(root, "current", &"a".repeat(40)).await;
    let path = root.join("versions/current/current.json");
    let mut value = read_json(&path).await.unwrap();
    value["data"] =
        json!({"PATCHED": {"client": "[example:patched:1.0]", "server": ""}});
    value["processors"] = json!([
        {"jar":"example:processor:1.0", "classpath":["example:helper:1.0"], "args":["--input", "{MINECRAFT_JAR}", "--output", "{PATCHED}", "--other", "{LIBRARY_DIR}/example/generated/1.0/generated-1.0.jar", "--input={LIBRARY_DIR}/example/option/1.0/option-1.0.jar"], "outputs":{"{PATCHED}":"hash"}},
        {"jar":"example:server:1.0", "classpath":[], "args":["{UNKNOWN_SERVER_VALUE}"], "sides":["server"]}
    ]);
    fs::write(path, serde_json::to_vec(&value).unwrap())
        .await
        .unwrap();
    let mut refs = References::default();
    version_references(root, "current", &mut refs)
        .await
        .unwrap();
    assert!(refs.warnings.is_empty());
    assert!(!refs.uncertain_categories.contains("libraries"));
    for path in [
        "libraries/example/processor/1.0/processor-1.0.jar",
        "libraries/example/helper/1.0/helper-1.0.jar",
        "libraries/example/patched/1.0/patched-1.0.jar",
        "libraries/example/generated/1.0/generated-1.0.jar",
        "libraries/example/option/1.0/option-1.0.jar",
    ] {
        assert_eq!(refs.used(path), Some(true), "{path}");
    }
    assert_eq!(
        refs.used("libraries/example/unused/1.0/unused-1.0.jar"),
        Some(false)
    );
}

#[tokio::test]
async fn missing_modern_profile_does_not_block_verified_unused_legacy_copies() {
    use sha1_smol::Sha1;
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let root = directories.metadata_dir();
    fs::create_dir_all(root.join("assets/indexes"))
        .await
        .unwrap();
    fs::create_dir_all(root.join("resources/sounds"))
        .await
        .unwrap();
    let bytes = b"old game audio";
    let hash = Sha1::from(bytes.as_slice()).hexdigest();
    fs::write(root.join("resources/sounds/old.ogg"), bytes)
        .await
        .unwrap();
    fs::write(root.join("resources/user-notes.txt"), b"keep")
        .await
        .unwrap();
    fs::write(root.join("assets/indexes/legacy.json"), serde_json::to_vec(&json!({"objects": {"sounds/old.ogg": {"hash": hash, "size": bytes.len()}}})).unwrap()).await.unwrap();
    let mut refs = References::default();
    refs.manifests.insert("1.21.11-loader".into());
    refs.games.insert("1.21.11-loader".into(), "1.21.11".into());
    resolve_manifests(&root, &mut refs).await;
    assert!(refs.uncertain_categories.contains("libraries"));
    assert!(!refs.uncertain_categories.contains("resources"));
    let mut candidates = vec![];
    let mut groups = BTreeMap::new();
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &refs,
        &mut groups,
        &mut candidates,
        &mut vec![],
    )
    .unwrap();
    assert_eq!(groups["resources"].unused, bytes.len() as u64);
    assert_eq!(groups["resources"].unknown, 4);
    let result = remove_candidates(
        &directories,
        candidates,
        &HashSet::from(["resources".into()]),
        &refs,
    )
    .unwrap();
    assert_eq!(result.removed_bytes, bytes.len() as u64);
    assert!(root.join("resources/user-notes.txt").exists());
}

#[tokio::test]
async fn missing_legacy_profile_still_protects_old_resources() {
    let temp = tempfile::tempdir().unwrap();
    let mut refs = References::default();
    refs.manifests.insert("1.6.4-loader".into());
    refs.games.insert("1.6.4-loader".into(), "1.6.4".into());
    resolve_manifests(temp.path(), &mut refs).await;
    assert!(refs.uncertain_categories.contains("resources"));
    assert!(!uses_modern_resources("13w38a"));
    assert!(!uses_modern_resources("custom"));
}

#[tokio::test]
async fn legacy_cleanup_keeps_modified_files_even_when_the_length_matches() {
    use sha1_smol::Sha1;
    let temp = tempfile::tempdir().unwrap();
    let directories = directories(temp.path());
    let root = directories.metadata_dir();
    fs::create_dir_all(root.join("resources/sounds"))
        .await
        .unwrap();
    fs::write(root.join("resources/sounds/old.ogg"), b"user")
        .await
        .unwrap();
    let mut refs = References::default();
    refs.legacy_candidates.insert(
        "resources/sounds/old.ogg".into(),
        vec![(4, Sha1::from(b"game".as_slice()).hexdigest())],
    );
    let mut groups = BTreeMap::new();
    let mut candidates = vec![];
    collect(
        &opened_root(&directories).unwrap(),
        "",
        &refs,
        &mut groups,
        &mut candidates,
        &mut vec![],
    )
    .unwrap();
    assert!(candidates.is_empty());
    assert_eq!(groups["resources"].unknown, 4);
}

#[tokio::test]
async fn missing_loader_uses_the_local_base_asset_index_without_marking_all_assets_unknown()
 {
    let temp = tempfile::tempdir().unwrap();
    let hash = "a".repeat(40);
    fixture(temp.path(), "1.21.11", &hash).await;
    let mut refs = References::default();
    refs.manifests.insert("1.21.11-loader".into());
    refs.games.insert("1.21.11-loader".into(), "1.21.11".into());
    resolve_manifests(temp.path(), &mut refs).await;
    assert!(refs.uncertain_categories.contains("libraries"));
    assert!(!refs.uncertain_categories.contains("assets"));
    assert_eq!(refs.used(&format!("assets/objects/aa/{hash}")), Some(true));
    assert_eq!(
        refs.used(&format!("assets/objects/bb/{}", "b".repeat(40))),
        Some(false)
    );
}
