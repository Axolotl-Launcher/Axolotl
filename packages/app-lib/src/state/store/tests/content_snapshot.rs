//! What a content snapshot reads, and the transaction it reads it in.

use crate::state::instances::commands::content_snapshot::local;
use crate::state::store::content_snapshot::{get, get_with_saved_pack, pause};
use crate::state::store::derived_db::{DerivedDb, FILE_NAME};
use crate::state::store::{Collection as _, Handle, content_refresh_checks::*};

async fn seed(pool: &sqlx::SqlitePool) {
    sqlx::raw_sql(
        "INSERT INTO instances (id, path, name, install_stage, launcher_feature_version, created, modified) VALUES ('instance', 'example', 'Example', 'installed', '1', 1, 1);
         INSERT INTO instance_content_sets (id, instance_id, name, source_kind, status, game_version, loader, revision, created, modified) VALUES ('set', 'instance', 'Main', 'local', 'available', '1.21', 'fabric', 1, 1, 1);
         UPDATE instances SET applied_content_set_id = 'set' WHERE id = 'instance';
         INSERT INTO instance_files (id, instance_id, relative_path, file_name, enabled, sha1, size, added_at, modified_at) VALUES ('file', 'instance', 'mods/a.jar', 'a.jar', 1, 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 3, 1, 1);
         INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, project_type, source_kind, server_requirement, client_requirement, enabled, added_at, modified_at) VALUES ('entry', 'instance', 'set', 'file', 'mod', 'local', 'optional', 'optional', 1, 1, 1);"
    ).execute(pool).await.unwrap();
}

/// A database of its own, with every migration applied and one instance in it.
async fn seeded() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    seed(&pool).await;
    pool
}

/// A derived database of its own, with every derived migration applied.
async fn derived() -> (tempfile::TempDir, DerivedDb) {
    let directory = tempfile::tempdir().unwrap();
    let db = DerivedDb::open(directory.path().join(FILE_NAME)).await;
    (directory, db)
}

/// The revision and the items come from one read transaction: a write that lands
/// between the two reads is in neither. The pause the snapshot offers a test
/// build is what makes the second read happen after the write, so the snapshot
/// mixing two reads would read the new revision beside the old items.
#[tokio::test]
async fn snapshot_revision_and_items_stay_in_the_same_read_transaction() {
    let temp = tempfile::tempdir().unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(temp.path().join("app.db"))
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    seed(&pool).await;
    let handle = Handle::new(pool.clone());

    let mut writer = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM instance_files WHERE id = 'file'")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE instance_content_sets SET revision = 2 WHERE id = 'set'",
    )
    .execute(&mut *writer)
    .await
    .unwrap();
    let _paused = pause::pause(Box::pin(async move {
        writer.commit().await.unwrap();
    }));

    let records = get(&handle, "instance").await.unwrap();
    assert_eq!(
        records.content_set.revision, 1,
        "the write that landed after the revision was read is not in it"
    );
    assert_eq!(
        records.files.len(),
        1,
        "and the items come from the same read, so they do not name it either"
    );

    let records = get(&handle, "instance").await.unwrap();
    assert_eq!(records.content_set.revision, 2, "the write did land");
    assert!(records.files.is_empty());
    pool.close().await;
}

#[tokio::test]
async fn local_snapshot_does_not_scan_direct_instances_or_require_a_game_directory()
 {
    let pool = seeded().await;
    sqlx::query("UPDATE instances SET linked_dot_minecraft = 'Z:/nonexistent/minecraft', linked_launcher = 'generic' WHERE id = 'instance'").execute(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());
    let records = get(&handle, "instance").await.unwrap();
    assert!(records.instance.is_direct_linked());
    assert_eq!(records.files.len(), 1);
    let snapshot = local::project(records);
    assert_eq!(snapshot.revision, 1);
    assert_eq!(snapshot.items.len(), 1);
    assert_eq!(
        snapshot.items[0].content.as_ref().unwrap().file_name,
        "a.jar"
    );
    pool.close().await;
}

#[tokio::test]
async fn local_projection_restores_success_after_failure_and_rejects_changed_conditions()
 {
    let pool = seeded().await;
    let handle = Handle::new(pool.clone());
    let records = get(&handle, "instance").await.unwrap();
    let mut record = RefreshCheckRecord {
        file_id: "file".into(),
        context: RefreshCheckContext {
            location_key: location_key(&records.instance),
            content_root: "E:/game".into(),
            sha1: "a".repeat(40),
            size: 3,
            current_release: None,
            preferred_source: records.preferred_source,
            target: crate::resources::ipc::ResourceTarget {
                game_versions: vec!["1.21".into()],
                loaders: vec!["fabric".into()],
                channels: crate::resources::plans::update_channels(
                    records.instance.update_channel,
                ),
            },
        },
        last_success: None,
        attempted_at: 1,
        attempt: CheckAttempt::Unrecognized,
        error: None,
    };
    let candidate: crate::resources::SourceReleaseRef = serde_json::from_value(serde_json::json!({ "project": { "source": "curseforge", "projectId": "123" }, "releaseId": "456" })).unwrap();
    record.succeeded(1, Some(candidate.clone()));
    record.failed(2, "offline".into());
    let key = RefreshCheckKey {
        instance_id: "instance".into(),
        entry_id: "entry".into(),
    };
    ContentRefreshChecks::set(&handle, &key, Some(&record))
        .await
        .unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert_eq!(snapshot.items[0].resource_update, Some(candidate));
    assert!(snapshot.items[0].capabilities.can_update);
    assert_eq!(snapshot.warnings.len(), 1);
    sqlx::query("UPDATE instances SET game_dir_override = 'E:/another-game' WHERE id = 'instance'").execute(&pool).await.unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert!(snapshot.items[0].resource_update.is_none());
    assert!(!snapshot.items[0].capabilities.can_update);
    sqlx::query("DELETE FROM instance_files WHERE id = 'file'")
        .execute(&pool)
        .await
        .unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert!(snapshot.items.is_empty());
    pool.close().await;
}

#[tokio::test]
async fn local_author_survives_an_older_presentation_without_an_owner() {
    use crate::state::store::content_refresh_presentations::*;
    let pool = seeded().await;
    let metadata =
        serde_json::json!({"mod_id":"example", "authors":["LocalAuthor"]});
    sqlx::query(
        "UPDATE instance_files SET local_mod_data = ? WHERE id = 'file'",
    )
    .bind(metadata.to_string())
    .execute(&pool)
    .await
    .unwrap();
    let handle = Handle::new(pool.clone());
    let records = get(&handle, "instance").await.unwrap();
    let location = location_key(&records.instance);
    let mut snapshot = local::project(records);
    let mut content = snapshot.items.remove(0).content.unwrap();
    assert_eq!(content.owner.as_ref().unwrap().name, "LocalAuthor");
    content.owner = None;
    ContentRefreshPresentations::set(
        &handle,
        &PresentationKey {
            instance_id: "instance".into(),
            file_id: "file".into(),
        },
        Some(&PresentationRecord {
            location_key: location,
            saved_at: 123,
            content,
        }),
    )
    .await
    .unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert_eq!(
        snapshot.items[0]
            .content
            .as_ref()
            .unwrap()
            .owner
            .as_ref()
            .unwrap()
            .name,
        "LocalAuthor"
    );
    pool.close().await;
}

#[tokio::test]
async fn local_declarations_fill_remote_unknown_dimensions_without_overriding_mismatches()
 {
    use crate::resources::CompatibilityState::{
        Compatible, Incompatible, Unknown,
    };
    let pool = seeded().await;
    sqlx::query(
        "UPDATE instance_files SET local_mod_data = ? WHERE id = 'file'",
    )
    .bind(serde_json::json!({"mod_id":"example", "minecraft_version":"[1.21,)", "loader":"fabric"}).to_string())
    .execute(&pool).await.unwrap();
    let handle = Handle::new(pool.clone());
    for (game_versions, loaders, expected_game, expected_loader) in [
        (vec![], vec![], Compatible, Compatible),
        (
            vec!["opaque range"],
            vec!["forge"],
            Compatible,
            Incompatible,
        ),
        (vec!["1.20"], vec![], Incompatible, Compatible),
    ] {
        let mut records = get(&handle, "instance").await.unwrap();
        records.artifacts.insert("entry".into(), serde_json::from_value(serde_json::json!({
            "reference": {"release": {"project": {"source":"modrinth", "projectId":"example1"}, "releaseId":"release1"}, "artifactId":"artifact1"},
            "fileName":"a.jar", "role":"primary", "available":true,
            "integrity":{"size":3,"hashes":[{"algorithm":"sha1","value":"a".repeat(40)}]},
            "acquisition":{"kind":"publicDownload","urls":["https://example.com/a.jar"]},
            "dependencies":[],
            "declaredCompatibility":{"gameVersions":game_versions,"loaders":loaders}
        })).unwrap());
        let snapshot = local::project(records);
        let assessment = snapshot.items[0].compatibility.as_ref().unwrap();
        assert_eq!(assessment.game_version, expected_game);
        assert_eq!(assessment.loader, expected_loader);
        assert_eq!(
            assessment.reasons.contains(&"gameVersion".into()),
            expected_game != Compatible
        );
        assert_eq!(
            assessment.reasons.contains(&"loader".into()),
            expected_loader != Compatible
        );
        assert_ne!(assessment.game_version, Unknown);
    }
    pool.close().await;
}

#[tokio::test]
async fn presentation_is_restored_only_for_matching_bytes_and_location() {
    use crate::state::store::content_refresh_presentations::*;
    let pool = seeded().await;
    let handle = Handle::new(pool.clone());
    let records = get(&handle, "instance").await.unwrap();
    let location_key = location_key(&records.instance);
    let mut snapshot = local::project(records);
    let mut content = snapshot.items.remove(0).content.unwrap();
    content.project.as_mut().unwrap().title = "Saved project title".into();
    let key = PresentationKey {
        instance_id: "instance".into(),
        file_id: "file".into(),
    };
    ContentRefreshPresentations::set(
        &handle,
        &key,
        Some(&PresentationRecord {
            location_key,
            saved_at: 123,
            content,
        }),
    )
    .await
    .unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert_eq!(
        snapshot.items[0]
            .content
            .as_ref()
            .unwrap()
            .project
            .as_ref()
            .unwrap()
            .title,
        "Saved project title"
    );
    sqlx::query("UPDATE instance_files SET sha1 = ? WHERE id = 'file'")
        .bind("b".repeat(40))
        .execute(&pool)
        .await
        .unwrap();
    let snapshot = local::project(get(&handle, "instance").await.unwrap());
    assert_ne!(
        snapshot.items[0]
            .content
            .as_ref()
            .unwrap()
            .project
            .as_ref()
            .unwrap()
            .title,
        "Saved project title"
    );
    sqlx::query("DELETE FROM instance_files WHERE id = 'file'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        ContentRefreshPresentations::get_all(&handle)
            .await
            .unwrap()
            .is_empty()
    );
    pool.close().await;
}

#[tokio::test]
async fn malformed_optional_cache_does_not_hide_the_local_file() {
    let pool = seeded().await;
    sqlx::query("INSERT INTO content_refresh_presentations(instance_id, file_id, payload) VALUES ('instance', 'file', 'broken json')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO content_refresh_checks(instance_id, entry_id, file_id, payload) VALUES ('instance', 'entry', 'file', '{}')").execute(&pool).await.unwrap();
    let records = get(&Handle::new(pool.clone()), "instance").await.unwrap();
    assert!(records.presentations.is_empty());
    assert!(records.checks.is_empty());
    assert_eq!(records.files.len(), 1);
    pool.close().await;
}

/// The saved pack left the app database, so the snapshot no longer reads it
/// inside its transaction: `get_with_saved_pack` reads it from the derived
/// database once the rest is read. Its foreign key to the content set left
/// with the table too, so a set that goes away no longer takes the row with
/// it, and the deletion of the instance is what drops it.
#[tokio::test]
async fn saved_pack_metadata_recovers_without_network_and_outlives_the_content_set()
 {
    use crate::state::store::content_refresh_pack::*;
    let pool = seeded().await;
    let handle = Handle::new(pool.clone());
    let (_directory, derived) = derived().await;
    let records = get(&handle, "instance").await.unwrap();
    let key = PackKey {
        instance_id: "instance".into(),
        content_set_id: "set".into(),
    };
    let pack = crate::state::InstanceContentPack {
        resource_id: None,
        name: "Saved pack".into(),
        icon_path: None,
        provider: None,
        project_id: None,
        version_id: None,
        reconciled: true,
        can_update: false,
        metadata: None,
    };
    ContentRefreshPacks::upsert(
        &derived,
        &key,
        &PackRecord {
            location_key: location_key(&records.instance),
            link_key: serde_json::to_string(&records.link).unwrap(),
            saved_at: 10,
            pack: Some(pack),
            attempted_at: 10,
            last_error: None,
        },
    )
    .await
    .unwrap();
    let snapshot = local::project(
        get_with_saved_pack(&handle, &derived, "instance")
            .await
            .unwrap(),
    );
    assert_eq!(snapshot.pack.unwrap().name, "Saved pack");
    sqlx::query("UPDATE instances SET game_dir_override = 'changed' WHERE id = 'instance'").execute(&pool).await.unwrap();
    let snapshot = local::project(
        get_with_saved_pack(&handle, &derived, "instance")
            .await
            .unwrap(),
    );
    assert!(snapshot.pack.is_none());
    assert!(
        ContentRefreshPacks::saved(&derived, &key)
            .await
            .unwrap()
            .is_some(),
        "the record stays; the projection drops it because the location moved"
    );
    sqlx::query("UPDATE instances SET applied_content_set_id = NULL WHERE id = 'instance'").execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM instance_content_sets WHERE id = 'set'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        ContentRefreshPacks::saved(&derived, &key)
            .await
            .unwrap()
            .is_some(),
        "a content set no longer owns the record: the cascade went with it"
    );
    assert!(
        get_with_saved_pack(&handle, &derived, "instance")
            .await
            .is_err(),
        "without an applied content set there is no snapshot to project"
    );
    ContentRefreshPacks::forget_instance(&derived, "instance")
        .await
        .unwrap();
    assert!(
        ContentRefreshPacks::saved(&derived, &key)
            .await
            .unwrap()
            .is_none(),
        "deleting the instance is what drops its derived records now"
    );
    pool.close().await;
    derived.pool().close().await;
}
