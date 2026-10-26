use super::super::types::{ProjectionEntry, ProjectionOwner};
use super::super::{codec, limits, projection, reconciliation, storage};
use super::*;
use std::sync::Arc;

fn server(id: &str) -> CanonicalServer {
    CanonicalServer {
        id: id.into(),
        data: codec::server_data(id.into(), format!("{id}.example.test"), None),
    }
}

async fn fixture() -> (tempfile::TempDir, Arc<State>, InstanceMetadata) {
    let temp = tempfile::tempdir().unwrap();
    let dirs = crate::state::DirectoryInfo {
        settings_dir: temp.path().join("settings"),
        config_dir: temp.path().join("config"),
        app_identifier: "server-sync-test".into(),
    };
    std::fs::create_dir_all(dirs.instances_dir().join("a")).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified)
         VALUES ('a', 'a', 'installed', '1', 'A', 1, 1);
         INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
         VALUES ('set-a', 'a', 'A', 'local', 'available', '1.20.1', 'vanilla', 1, 1);
         UPDATE instances SET applied_content_set_id = 'set-a' WHERE id = 'a';
         INSERT INTO instance_sync_preferences(instance_id, feature, enabled)
         VALUES ('a', 'multiplayer_servers', 1)
         ON CONFLICT(instance_id, feature) DO UPDATE SET enabled = 1;"
    ).execute(&pool).await.unwrap();
    let state = crate::state::test_state(dirs, pool).await.unwrap();
    let metadata = crate::state::get_instance("a", &state.pool)
        .await
        .unwrap()
        .unwrap();
    (temp, state, metadata)
}

#[tokio::test]
async fn captured_change_survives_failed_publication_without_reimport() {
    let (_temp, state, metadata) = fixture().await;
    let root = instance_game_dir(&metadata, &state).unwrap();
    let original = vec![server("original")];
    commit_server_state(Some(&original), None, &state)
        .await
        .unwrap();
    publish_instance(&metadata, &root, &state).await.unwrap();
    let target = root.join(SERVERS_FILE);
    let current = vec![server("original").data, server("added").data];
    // Minecraft commonly replaces servers.dat rather than editing the link.
    projection::detach_existing_file(&target).await.unwrap();
    codec::write_servers(&target, &current).await.unwrap();
    let old_projection = load_projection_entries("a", &state).await.unwrap();
    let (next, locals) = reconciliation::reconcile_entries(
        current.clone(),
        &old_projection,
        original,
        vec![],
    );
    storage::commit_captured_servers("a", &next, &locals, &state)
        .await
        .unwrap();
    let generated = generated_path(&state, "a");
    std::fs::remove_file(&generated).unwrap();
    std::fs::create_dir(&generated).unwrap();
    assert!(publish_instance(&metadata, &root, &state).await.is_err());
    assert_eq!(
        checkpoint("a", SyncedOption::MultiplayerServers, "default", &state)
            .await
            .unwrap()
            .unwrap()
            .status,
        CheckpointStatus::Pending
    );
    assert_eq!(read_canonical(&state).await.unwrap().len(), 2);
    std::fs::remove_dir(&generated).unwrap();
    publish_instance(&metadata, &root, &state).await.unwrap();
    assert_eq!(read_servers(&target).await.unwrap(), current);
    assert_eq!(
        checkpoint("a", SyncedOption::MultiplayerServers, "default", &state)
            .await
            .unwrap()
            .unwrap()
            .status,
        CheckpointStatus::Ready
    );
    assert_eq!(read_canonical(&state).await.unwrap().len(), 2);
}

#[tokio::test]
async fn capture_and_pending_checkpoint_roll_back_together() {
    let (_temp, state, _) = fixture().await;
    commit_server_state(Some(&[server("original")]), None, &state)
        .await
        .unwrap();
    sqlx::query(
        "DELETE FROM instance_sync_preferences WHERE instance_id = 'a'",
    )
    .execute(&state.pool)
    .await
    .unwrap();
    assert!(
        storage::commit_captured_servers(
            "a",
            &[server("replacement")],
            &[],
            &state
        )
        .await
        .is_err()
    );
    assert_eq!(read_canonical(&state).await.unwrap()[0].id, "original");
}

#[tokio::test]
async fn reading_and_recommitting_nbt_does_not_increment_revision() {
    let (_temp, state, _) = fixture().await;
    commit_server_state(Some(&[server("original")]), None, &state)
        .await
        .unwrap();
    let revision = server_revision(&state).await.unwrap();
    for _ in 0..16 {
        let servers = read_canonical(&state).await.unwrap();
        assert!(
            !commit_server_state(Some(&servers), None, &state)
                .await
                .unwrap()
        );
    }
    assert_eq!(server_revision(&state).await.unwrap(), revision);
}

#[tokio::test]
async fn generated_file_provenance_requires_the_same_physical_directory() {
    let (temp, state, metadata) = fixture().await;
    commit_server_state(Some(&[server("original")]), None, &state)
        .await
        .unwrap();
    let root = instance_game_dir(&metadata, &state).unwrap();
    publish_instance(&metadata, &root, &state).await.unwrap();
    let actual = sha1_bytes(&std::fs::read(root.join(SERVERS_FILE)).unwrap());
    assert!(
        sibling_wrote_file("sibling", &root, &actual, &state)
            .await
            .unwrap()
    );
    assert!(
        !sibling_wrote_file("sibling", temp.path(), &actual, &state)
            .await
            .unwrap()
    );
    assert!(
        !sibling_wrote_file("sibling", &root, "user edited file", &state)
            .await
            .unwrap()
    );
    assert!(
        !sibling_wrote_file("a", &root, &actual, &state)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn oversized_database_list_is_preserved_and_rejected_before_decoding() {
    let (_temp, state, _) = fixture().await;
    sqlx::query("INSERT INTO synced_servers(id, position, nbt) VALUES ('oversized', 0, zeroblob(?))")
        .bind(limits::MAX_SERVER_BYTES as i64 + 1).execute(&state.pool).await.unwrap();
    let error = read_canonical(&state).await.err().unwrap();
    assert!(matches!(error.raw.as_ref(), ErrorKind::InputError(_)));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM synced_servers")
        .fetch_one(&state.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn stale_projection_does_not_multiply_a_sibling_import() {
    let original = server("original");
    let projections = vec![ProjectionEntry {
        id: original.id.clone(),
        owner: ProjectionOwner::Synced,
        data: original.data.clone(),
        position: 0,
    }];
    let mut canonical = vec![original, server("added")];
    for _ in 0..20 {
        let current =
            canonical.iter().map(|server| server.data.clone()).collect();
        (canonical, _) = reconciliation::reconcile_entries(
            current,
            &projections,
            canonical,
            vec![],
        );
        assert_eq!(canonical.len(), 2);
        assert_eq!(canonical[1].id, "added");
    }
}

#[test]
fn intentional_duplicate_and_deletion_keep_individual_identity() {
    let first = server("original");
    let projections = vec![ProjectionEntry {
        id: first.id.clone(),
        owner: ProjectionOwner::Synced,
        data: first.data.clone(),
        position: 0,
    }];
    let (duplicated, _) = reconciliation::reconcile_entries(
        vec![first.data.clone(), first.data.clone()],
        &projections,
        vec![first.clone()],
        vec![],
    );
    assert_eq!(duplicated.len(), 2);
    assert_ne!(duplicated[0].id, duplicated[1].id);
    let (deleted, _) = reconciliation::reconcile_entries(
        vec![],
        &projections,
        vec![first],
        vec![],
    );
    assert!(deleted.is_empty());
}

#[tokio::test]
async fn missing_directory_and_disabled_sync_do_not_create_projection() {
    let (_temp, state, metadata) = fixture().await;
    let root = instance_game_dir(&metadata, &state).unwrap();
    std::fs::remove_dir(&root).unwrap();
    compose_instance(&metadata, &state).await.unwrap();
    reconcile_servers(&metadata, &state).await.unwrap();
    assert!(!generated_path(&state, "a").exists());
    assert!(
        checkpoint("a", SyncedOption::MultiplayerServers, "default", &state)
            .await
            .unwrap()
            .is_none()
    );
    std::fs::create_dir(&root).unwrap();
    sqlx::query("UPDATE sync_feature_settings SET globally_enabled = 0 WHERE feature = 'multiplayer_servers'")
        .execute(&state.pool).await.unwrap();
    compose_instance(&metadata, &state).await.unwrap();
    reconcile_servers(&metadata, &state).await.unwrap();
    assert!(!root.join(SERVERS_FILE).exists());
    assert!(
        checkpoint("a", SyncedOption::MultiplayerServers, "default", &state)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn detach_does_not_restore_deleted_files_from_generated_cache() {
    let (_temp, state, metadata) = fixture().await;
    commit_server_state(Some(&[server("original")]), None, &state)
        .await
        .unwrap();
    let root = instance_game_dir(&metadata, &state).unwrap();
    publish_instance(&metadata, &root, &state).await.unwrap();
    std::fs::remove_file(root.join(SERVERS_FILE)).unwrap();
    detach_servers(&metadata, &state).await.unwrap();
    assert!(generated_path(&state, "a").exists());
    assert!(!root.join(SERVERS_FILE).exists());
    assert!(
        load_projection_entries("a", &state)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        checkpoint("a", SyncedOption::MultiplayerServers, "default", &state)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn detach_preserves_contents_and_breaks_hard_link() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("generated");
    let target = temp.path().join(SERVERS_FILE);
    std::fs::write(&source, b"existing data").unwrap();
    std::fs::hard_link(&source, &target).unwrap();
    projection::detach_existing_file(&target).await.unwrap();
    std::fs::write(source, b"new shared data").unwrap();
    assert_eq!(std::fs::read(target).unwrap(), b"existing data");
}

#[tokio::test]
async fn oversized_file_is_rejected_without_reading_or_replacing_it() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join(SERVERS_FILE);
    std::fs::File::create(&file)
        .unwrap()
        .set_len(limits::MAX_SERVER_BYTES as u64 + 1)
        .unwrap();
    assert!(read_servers(&file).await.is_err());
    assert_eq!(
        std::fs::metadata(file).unwrap().len(),
        limits::MAX_SERVER_BYTES as u64 + 1
    );
    assert!(
        codec::servers_to_bytes(&vec![
            server("original").data;
            limits::MAX_SERVERS + 1
        ])
        .is_err()
    );
}
