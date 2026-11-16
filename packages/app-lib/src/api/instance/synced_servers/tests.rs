use super::super::types::{ProjectionEntry, ProjectionOwner};
use super::super::{codec, limits, projection, reconciliation, storage};
use super::*;
use crate::state::store::instance_sync_preferences::{
    InstancePreferenceKey, InstancePreferenceRecord, InstanceSyncPreferences,
};
use crate::state::store::sync_feature_settings::{
    FeatureSettingsKey, FeatureSettingsRecord, SyncFeatureSettings,
};
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
    let settings = dirs.settings_dir.join("settings");
    std::fs::create_dir_all(&settings).unwrap();
    std::fs::write(
        settings.join("bootstrap.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "data": { "custom_dir": dirs.config_dir }
        }))
        .unwrap(),
    )
    .unwrap();
    let pool = crate::state::store::databases::open(&dirs.settings_dir)
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO instances(id, path, install_stage, launcher_feature_version, name, created, modified)
         VALUES ('a', 'a', 'installed', 'none', 'A', 1, 1);
         INSERT INTO instance_content_sets(id, instance_id, name, source_kind, status, game_version, loader, created, modified)
         VALUES ('set-a', 'a', 'A', 'local', 'available', '1.20.1', 'vanilla', 1, 1);
         UPDATE instances SET applied_content_set_id = 'set-a' WHERE id = 'a';"
    ).execute(&pool).await.unwrap();
    let state = crate::state::test_state(dirs, pool).await.unwrap();
    let mut unit = crate::state::store::atom(&state.data).await.unwrap();
    InstanceSyncPreferences::atom_set(
        &mut unit,
        &InstancePreferenceKey {
            instance_id: "a".into(),
            feature: "multiplayer_servers".into(),
        },
        Some(&InstancePreferenceRecord { enabled: true }),
    )
    .await
    .unwrap();
    SyncFeatureSettings::atom_set(
        &mut unit,
        &FeatureSettingsKey {
            feature: "multiplayer_servers".into(),
        },
        Some(&FeatureSettingsRecord {
            globally_enabled: true,
            new_instance_default: false,
        }),
    )
    .await
    .unwrap();
    unit.commit().await.unwrap();
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
    InstanceSyncPreferences::set(
        &state.data,
        &InstancePreferenceKey {
            instance_id: "a".to_string(),
            feature: "multiplayer_servers".to_string(),
        },
        None,
    )
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
async fn oversized_shared_native_source_is_preserved_and_rejected_before_decoding()
 {
    let (_temp, state, _) = fixture().await;
    let root =
        crate::state::store::sync_documents::authority(&state.persistence)
            .await
            .unwrap();
    let relative = "state/generations/oversized-servers.dat";
    let native = root.join(relative);
    std::fs::create_dir_all(native.parent().unwrap()).unwrap();
    std::fs::File::create(&native)
        .unwrap()
        .set_len(limits::MAX_SERVER_BYTES as u64 + 1)
        .unwrap();
    crate::state::sync_state::manifest::document(&root)
        .update(move |data| {
            data.insert(
                "native".into(),
                serde_json::json!({
                    "servers": {
                        "file": relative,
                        "bytes": limits::MAX_SERVER_BYTES + 1,
                        "sha256": "0".repeat(64)
                    }
                }),
            );
            Ok(())
        })
        .await
        .unwrap();
    let head = std::fs::read(root.join("state/head.json")).unwrap();
    let error = read_canonical(&state).await.err().unwrap();
    assert!(matches!(error.raw.as_ref(), ErrorKind::InputError(_)));
    assert!(error.to_string().contains("Invalid native sync reference"));
    assert_eq!(std::fs::read(root.join("state/head.json")).unwrap(), head);
    assert_eq!(
        std::fs::metadata(native).unwrap().len(),
        limits::MAX_SERVER_BYTES as u64 + 1
    );
}

#[tokio::test]
async fn oversized_server_write_does_not_replace_the_committed_source() {
    let (_temp, state, _) = fixture().await;
    commit_server_state(Some(&[server("original")]), None, &state)
        .await
        .unwrap();
    let revision = server_revision(&state).await.unwrap();
    let oversized = (0..=limits::MAX_SERVERS)
        .map(|index| server(&format!("server-{index}")))
        .collect::<Vec<_>>();
    assert!(
        commit_server_state(Some(&oversized), None, &state)
            .await
            .is_err()
    );
    assert_eq!(server_revision(&state).await.unwrap(), revision);
    let servers = read_canonical(&state).await.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id, "original");
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
    let key = FeatureSettingsKey {
        feature: "multiplayer_servers".to_string(),
    };
    let mut settings = SyncFeatureSettings::get(&state.data, &key)
        .await
        .unwrap()
        .unwrap();
    settings.globally_enabled = false;
    SyncFeatureSettings::set(&state.data, &key, Some(&settings))
        .await
        .unwrap();
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
