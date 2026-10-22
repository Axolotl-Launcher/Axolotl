use super::*;

#[test]
fn cache_totals_include_previous_location_without_counting_shared_roots_twice()
{
    let temp = tempfile::tempdir().unwrap();
    let mut directories = DirectoryInfo {
        settings_dir: temp.path().join("settings"),
        config_dir: temp.path().join("data"),
        app_identifier: "test".into(),
    };
    for (root, size) in [
        (&directories.settings_dir, 11),
        (&directories.config_dir, 13),
    ] {
        std::fs::create_dir_all(root.join("caches")).unwrap();
        std::fs::write(root.join("caches/file"), vec![0; size]).unwrap();
    }
    assert_eq!(cache_bytes(&directories, &mut vec![]).unwrap(), 24);
    directories.config_dir = directories.settings_dir.clone();
    assert_eq!(cache_bytes(&directories, &mut vec![]).unwrap(), 11);
}

#[test]
fn download_cleanup_keeps_runtime_files_and_referenced_content() {
    let temp = tempfile::tempdir().unwrap();
    let digest = "a".repeat(128);
    let orphan = "b".repeat(128);
    let files = [
        "content/modrinth/release/mod.jar".to_string(),
        "curseforge/modpacks/pack.zip".to_string(),
        "modpacks/project/pack.mrpack".to_string(),
        format!("content-store/{orphan}"),
        format!("content-store/{digest}"),
        "icons/instance.png".to_string(),
        "minecraft-natives/native.jar".to_string(),
        "resource-staging/job/file.jar".to_string(),
        "unknown/user.txt".to_string(),
        "resources/beta/resource-cache.db".to_string(),
    ];
    for file in &files {
        let path = temp.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"cache").unwrap();
    }
    let root = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority())
        .unwrap();
    let mut failures = vec![];
    assert_eq!(walk(&root, "", None, &mut failures).unwrap(), 50);
    walk(&root, "", Some(&HashSet::from([digest])), &mut failures).unwrap();
    assert!(failures.is_empty());
    assert_eq!(walk(&root, "", None, &mut failures).unwrap(), 30);
    for (index, file) in files.iter().enumerate() {
        assert_eq!(temp.path().join(file).exists(), index >= 4, "{file}");
    }
}

#[tokio::test]
async fn channel_references_keep_installed_retained_and_incomplete_store_objects()
 {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TABLE install_jobs(status TEXT, state TEXT); CREATE TABLE store_blobs(digest TEXT); CREATE TABLE store_instance_files(digest TEXT); CREATE TABLE store_retained_refs(digest TEXT); CREATE TABLE store_operations(digest TEXT, state TEXT); INSERT INTO store_instance_files VALUES ('INSTALLED'); INSERT INTO store_retained_refs VALUES ('HISTORY'); INSERT INTO store_operations VALUES ('PENDING','published'), ('DONE','materialized');").execute(&pool).await.unwrap();
    let retained: HashSet<_> = retained_in_channel(&pool)
        .await
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(
        retained,
        HashSet::from(["installed".into(), "history".into(), "pending".into()])
    );
    sqlx::query("INSERT INTO install_jobs VALUES ('waiting_for_user', '{}')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(retained_in_channel(&pool).await.is_err());
    sqlx::query("UPDATE install_jobs SET status = 'failed', state = '{\"rollback\":{}}'").execute(&pool).await.unwrap();
    assert!(retained_in_channel(&pool).await.is_err());
    pool.close().await;
}

#[tokio::test]
async fn old_channels_without_a_content_store_are_supported_but_partial_schemas_are_refused()
 {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE install_jobs(status TEXT, state TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(retained_in_channel(&pool).await.unwrap().is_empty());
    sqlx::query("CREATE TABLE store_blobs(digest TEXT)")
        .execute(&pool)
        .await
        .unwrap();
    assert!(retained_in_channel(&pool).await.is_err());
    pool.close().await;
}

#[cfg(unix)]
#[test]
fn cleanup_does_not_follow_links_outside_cache() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("keep.jar"), b"keep").unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("content"))
        .unwrap();
    let root = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority())
        .unwrap();
    walk(&root, "", Some(&HashSet::new()), &mut vec![]).unwrap();
    assert!(outside.path().join("keep.jar").exists());
}
