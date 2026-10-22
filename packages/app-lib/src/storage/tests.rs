use super::*;

#[cfg(windows)]
#[tokio::test]
async fn lowercase_backup_directory_is_not_counted_again_as_other() {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().join("settings"),
        config_dir: temp.path().join("data"),
        app_identifier: "test".into(),
    };
    let actual_root = directories.config_dir.join("backups");
    let actual_active = actual_root.join("instances");
    fs::create_dir_all(actual_root.join("objects/aa"))
        .await
        .unwrap();
    fs::create_dir_all(&actual_active).await.unwrap();
    fs::create_dir_all(&directories.settings_dir).await.unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(actual_root.join("backup.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE repository_meta(id INTEGER); CREATE TABLE backup_objects(id INTEGER); CREATE TABLE backup_snapshots(id INTEGER);").execute(&pool).await.unwrap();
    pool.close().await;
    fs::write(actual_root.join("objects/aa/blob"), [0; 100])
        .await
        .unwrap();
    fs::write(actual_active.join("backup.db"), [0; 19])
        .await
        .unwrap();
    fs::write(directories.config_dir.join("unclassified.txt"), [0; 7])
        .await
        .unwrap();
    let expected = fs::metadata(actual_root.join("backup.db"))
        .await
        .unwrap()
        .len()
        + 119;
    let tree = scan_with_directories(
        &directories,
        &directories.config_dir.join("Backups/instances"),
        vec![],
    )
    .await
    .unwrap();
    let backups = tree
        .categories
        .iter()
        .find(|node| node.node_type == StorageNodeType::Backups)
        .unwrap();
    assert_eq!(backups.size.actual, expected);
    assert_eq!(tree.total.actual, expected + 7);
    let other = tree.root_other.unwrap();
    assert_eq!(other.size.actual, 7);
    assert_eq!(
        other.children.unwrap()[0].name.as_deref(),
        Some("unclassified.txt")
    );
    assert!(tree.warnings.is_empty());
}

#[tokio::test]
async fn scan_reports_completed_areas_and_cache_retains_stale_results_for_display()
 {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "test".into(),
    };
    fs::create_dir_all(directories.instances_dir())
        .await
        .unwrap();
    fs::write(directories.instances_dir().join("test.jar"), b"game")
        .await
        .unwrap();
    let events = std::sync::Mutex::new(Vec::new());
    let mut tree = scan_with_directories_progress(
        &directories,
        &temp.path().join("backups"),
        vec![],
        &|event| events.lock().unwrap().push(event),
    )
    .await
    .unwrap();
    let events = events.into_inner().unwrap();
    let steps: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            StorageScanEvent::Progress {
                completed, total, ..
            } => Some((*completed, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(steps[0].0, 0);
    assert!(steps[0].1 >= 16);
    let last = steps.last().unwrap();
    assert_eq!(last.0, last.1);
    assert!(
        steps
            .windows(2)
            .all(|pair| pair[1].0 >= pair[0].0 && pair[1].1 == pair[0].1)
    );
    assert!(snapshot_is_fresh(&tree));
    tree.scanned_at = Utc::now() - chrono::Duration::hours(1);
    assert!(!snapshot_is_fresh(&tree));
    let snapshot = StorageCacheFile {
        version: STORAGE_CACHE_VERSION,
        roots: "current-roots".into(),
        tree: tree.clone(),
    };
    let bytes = serde_cbor::to_vec(&snapshot).unwrap();
    assert_eq!(
        decode_snapshot(&bytes, "current-roots")
            .unwrap()
            .total
            .actual,
        tree.total.actual
    );
    assert!(decode_snapshot(&bytes, "different-roots").is_none());
    assert!(decode_snapshot(b"invalid cache", "current-roots").is_none());
    let old = StorageCacheFile {
        version: STORAGE_CACHE_VERSION - 1,
        ..snapshot
    };
    assert!(
        decode_snapshot(&serde_cbor::to_vec(&old).unwrap(), "current-roots")
            .is_none()
    );
    tree.scanned_at = Utc::now() + chrono::Duration::hours(1);
    assert!(!snapshot_is_fresh(&tree));
}

#[tokio::test]
async fn nested_roots_and_external_backup_are_counted_once() {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().join("settings"),
        config_dir: temp.path().join("settings/data"),
        app_identifier: "test".into(),
    };
    let backup = temp.path().join("backups");
    for (path, size) in [
        (
            directories.config_dir.join("profiles/example/mods/a.jar"),
            11,
        ),
        (directories.config_dir.join("caches/content-store/hash"), 13),
        (
            directories
                .config_dir
                .join("servers/server/world/region/a.mca"),
            17,
        ),
        (
            directories.config_dir.join("meta/assets/objects/aa/hash"),
            19,
        ),
        (directories.settings_dir.join("release/app.db"), 23),
        (backup.join("objects/aa/blob"), 29),
        (directories.settings_dir.join("unclassified.txt"), 31),
    ] {
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();
        fs::write(path, vec![1; size]).await.unwrap();
    }
    let tree = scan_with_directories(&directories, &backup, vec![])
        .await
        .unwrap();
    assert_eq!(tree.total.actual, 143);
    assert!(tree.warnings.is_empty());
    assert_eq!(
        tree.categories
            .iter()
            .find(|node| node.node_type == StorageNodeType::Backups)
            .unwrap()
            .size
            .actual,
        29
    );
    assert_eq!(tree.root_other.unwrap().size.actual, 31);
}

#[tokio::test]
async fn backup_inside_another_category_has_exclusive_ownership() {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "test".into(),
    };
    let backup = directories.caches_dir().join("backups");
    fs::create_dir_all(&backup).await.unwrap();
    fs::write(backup.join("blob"), [0; 32]).await.unwrap();
    let tree = scan_with_directories(&directories, &backup, vec![])
        .await
        .unwrap();
    assert_eq!(tree.total.actual, 32);
    assert_eq!(tree.categories.len(), 1);
    assert_eq!(tree.categories[0].node_type, StorageNodeType::Backups);
}

#[tokio::test]
async fn a_link_at_the_scan_root_never_counts_as_owned_bytes() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    let link = temp.path().join("link");
    fs::create_dir_all(&target).await.unwrap();
    fs::write(target.join("data"), [0; 24]).await.unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &link).unwrap();
    #[cfg(windows)]
    junction::create(&target, &link).unwrap();
    let node = scan(&link, 2, &HashSet::new(), &mut vec![]).await.unwrap();
    assert_eq!(
        node.size,
        StorageSize {
            actual: 0,
            symlink: 24
        }
    );
}

#[tokio::test]
async fn scan_errors_do_not_discard_readable_siblings() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("files");
    fs::create_dir_all(&path).await.unwrap();
    fs::write(path.join("valid"), [0; 7]).await.unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        temp.path().join("missing"),
        path.join("broken"),
    )
    .unwrap();
    let mut warnings = vec![];
    let node = scan(&path, 1, &HashSet::new(), &mut warnings)
        .await
        .unwrap();
    assert_eq!(node.size.actual, 7);
    #[cfg(unix)]
    assert_eq!(warnings.len(), 1);
}

#[tokio::test]
async fn database_category_has_release_and_beta_as_direct_children() {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "test".into(),
    };
    for channel in ["release", "beta"] {
        fs::create_dir_all(temp.path().join(channel)).await.unwrap();
        fs::write(temp.path().join(channel).join("app.db"), [0; 9])
            .await
            .unwrap();
    }
    let tree = scan_with_directories(
        &directories,
        &temp.path().join("backups"),
        vec![],
    )
    .await
    .unwrap();
    let database = tree
        .categories
        .iter()
        .find(|node| node.node_type == StorageNodeType::Database)
        .unwrap();
    assert!(database.name.is_none());
    let names: HashSet<_> = database
        .children
        .as_ref()
        .unwrap()
        .iter()
        .map(|node| node.name.as_deref().unwrap())
        .collect();
    assert_eq!(names, HashSet::from(["release", "beta"]));
    assert_eq!(database.size.actual, 18);
}

#[tokio::test]
async fn legacy_repository_and_launcher_files_are_classified_without_root_wrappers()
 {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().join("red.ghs.axolotl"),
        config_dir: temp.path().join("data"),
        app_identifier: "test".into(),
    };
    let legacy = directories.config_dir.join("Backups");
    let active = legacy.join("instances");
    for root in [&legacy, &active] {
        fs::create_dir_all(root.join("objects/aa")).await.unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.join("backup.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE repository_meta(id INTEGER); CREATE TABLE backup_objects(id INTEGER); CREATE TABLE backup_snapshots(id INTEGER);").execute(&pool).await.unwrap();
        pool.close().await;
    }
    for (path, size) in [
        (legacy.join("objects/aa/old-blob"), 100),
        (active.join("objects/aa/new-blob"), 200),
        (directories.settings_dir.join("caches/old-cache"), 13),
        (directories.config_dir.join("caches/new-cache"), 17),
        (directories.settings_dir.join("hongshi/component"), 19),
        (directories.settings_dir.join("backgrounds/image.png"), 23),
        (
            directories
                .settings_dir
                .join("instance-shortcut-icons/icon.ico"),
            29,
        ),
        (directories.settings_dir.join("unknown.txt"), 31),
        (directories.settings_dir.join("beta old/app.db"), 37),
    ] {
        fs::create_dir_all(path.parent().unwrap()).await.unwrap();
        fs::write(path, vec![0; size]).await.unwrap();
    }
    let expected_backup =
        fs::metadata(legacy.join("backup.db")).await.unwrap().len()
            + fs::metadata(active.join("backup.db")).await.unwrap().len()
            + 300;
    let tree = scan_with_directories(&directories, &active, vec![])
        .await
        .unwrap();
    let category_bytes = |kind| {
        tree.categories
            .iter()
            .find(|node| node.node_type == kind)
            .unwrap()
            .size
            .actual
    };
    assert_eq!(category_bytes(StorageNodeType::Backups), expected_backup);
    assert_eq!(category_bytes(StorageNodeType::Cache), 30);
    assert_eq!(category_bytes(StorageNodeType::Components), 19);
    assert_eq!(category_bytes(StorageNodeType::LauncherData), 23);
    assert_eq!(category_bytes(StorageNodeType::Icons), 29);
    assert_eq!(
        tree.total.actual,
        expected_backup + 30 + 19 + 23 + 29 + 31 + 37
    );
    let other = tree.root_other.unwrap();
    assert_eq!(other.size.actual, 68);
    let names: HashSet<_> = other
        .children
        .unwrap()
        .into_iter()
        .filter_map(|node| node.name)
        .collect();
    assert_eq!(
        names,
        HashSet::from(["unknown.txt".into(), "beta old".into()])
    );
    assert!(tree.warnings.is_empty());
}

#[tokio::test]
async fn backup_directory_name_alone_does_not_claim_unrecognized_files() {
    let temp = tempfile::tempdir().unwrap();
    let directories = DirectoryInfo {
        settings_dir: temp.path().into(),
        config_dir: temp.path().into(),
        app_identifier: "test".into(),
    };
    let root = temp.path().join("Backups");
    fs::create_dir_all(&root).await.unwrap();
    fs::write(root.join("backup.db"), b"not a backup database")
        .await
        .unwrap();
    fs::write(root.join("user.txt"), b"keep").await.unwrap();
    assert!(!is_backup_repository(&root).await);
    let tree =
        scan_with_directories(&directories, &root.join("instances"), vec![])
            .await
            .unwrap();
    assert!(
        tree.categories
            .iter()
            .all(|node| node.node_type != StorageNodeType::Backups)
    );
    assert_eq!(tree.root_other.unwrap().size.actual, 25);
}
