//! Domain handover and failure isolation using disposable databases only.
use crate::state::{
    persistence::Persistence,
    store::{self, Handle},
    user_state::{self, favorites, shared_logs::SharedLogs, worlds},
};
use crate::worlds::{DisplayStatus, WorldType};
use serde_json::json;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

async fn snapshot(root: &std::path::Path) -> std::path::PathBuf {
    let path = root.join("source.db");
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE attached_world_data(instance_id TEXT,world_type TEXT,world_id TEXT,display_status TEXT,project_id TEXT,content_kind TEXT);CREATE TABLE join_log(instance_id TEXT,host TEXT,port INTEGER,join_time INTEGER);CREATE TABLE resource_favorites(source TEXT,project_id TEXT,content_type TEXT,saved_at INTEGER);CREATE TABLE instances(id TEXT,pinned_at INTEGER);CREATE TABLE shared_logs(id TEXT,url TEXT,raw TEXT,token TEXT,provider TEXT,instance_id TEXT,instance_name TEXT,truncated INTEGER,created_at INTEGER);INSERT INTO attached_world_data VALUES('instance','singleplayer','world','favorite','project','world_save');INSERT INTO join_log VALUES('instance','server.example',25565,111);INSERT INTO resource_favorites VALUES('modrinth','project','mod',123);INSERT INTO instances VALUES('instance',456);INSERT INTO shared_logs VALUES('log','https://example.invalid/log','https://example.invalid/raw','deletion-secret','logshare','instance','Example',0,789);").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    path
}

#[tokio::test]
async fn imports_choices_and_preserves_unknown_fields_and_independent_failures()
{
    let temp = tempfile::tempdir().unwrap();
    let path = snapshot(temp.path()).await;
    store::legacy_user_state::initialize(temp.path(), Some(path)).await;
    let persistence = Persistence::new(temp.path());
    let data = worlds::get_all(&persistence, "instance").await.unwrap();
    assert_eq!(
        data[&(WorldType::Singleplayer, "world".into())].display_status,
        DisplayStatus::Favorite
    );
    assert_eq!(favorites::all(&persistence).await.unwrap()[0].saved_at, 123);
    let joins = user_state::joins::domain(&persistence)
        .read()
        .await
        .unwrap();
    assert_eq!(
        joins["entries"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["join_time"],
        111
    );

    assert_eq!(
        favorites::pin_values(&persistence).await["instance"].timestamp(),
        456
    );
    worlds::domain(&persistence)
        .update(|entries| {
            for raw in entries.values_mut() {
                raw["future_hint"] = json!({"retain":true});
            }
            Ok(())
        })
        .await
        .unwrap();
    worlds::set(
        &persistence,
        "instance",
        WorldType::Singleplayer,
        "world",
        "display_status",
        json!("hidden"),
    )
    .await
    .unwrap();
    assert_eq!(
        worlds::domain(&persistence).read().await.unwrap()["entries"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["future_hint"],
        json!({"retain":true})
    );
    let document = worlds::domain(&persistence).document;
    tokio::fs::write(document.path(), b"broken").await.unwrap();
    let previous = std::path::PathBuf::from(format!(
        "{}.previous",
        document.path().display()
    ));
    let _ = tokio::fs::remove_file(previous).await;
    assert!(worlds::get_all(&persistence, "instance").await.is_err());
    assert_eq!(favorites::all(&persistence).await.unwrap().len(), 1);
    assert!(
        worlds::set(
            &persistence,
            "instance",
            WorldType::Singleplayer,
            "world",
            "display_status",
            json!("normal")
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn completed_import_never_resurrects_deleted_or_unknown_favorites() {
    let temp = tempfile::tempdir().unwrap();
    let path = snapshot(temp.path()).await;
    store::legacy_user_state::initialize(temp.path(), Some(path.clone())).await;
    let persistence = Persistence::new(temp.path());
    favorites::remove(
        &persistence,
        vec![("modrinth".into(), "project".into())],
    )
    .await
    .unwrap();
    store::legacy_user_state::initialize(temp.path(), Some(path.clone())).await;
    assert!(favorites::all(&persistence).await.unwrap().is_empty());
    favorites::domain(&persistence).update(|entries|{entries.insert("opaque".into(),json!({"source":"future-provider","project_id":"future","content_type":"future-type","saved_at":10,"schemaVersion":2}));Ok(())}).await.unwrap();
    let before =
        tokio::fs::read(favorites::domain(&persistence).document.path())
            .await
            .unwrap();
    favorites::save(
        &persistence,
        favorites::Favorite {
            source: "modrinth".into(),
            project_id: "new".into(),
            content_type: "mod".into(),
            saved_at: 20,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        favorites::domain(&persistence).read().await.unwrap()["entries"]["opaque"]
            ["schemaVersion"],
        2
    );
    assert!(!before.is_empty());
    let target = favorites::domain(&persistence).document;
    tokio::fs::remove_file(target.path()).await.unwrap();
    let _ =
        tokio::fs::remove_file(format!("{}.previous", target.path().display()))
            .await;
    store::legacy_user_state::initialize(temp.path(), Some(path)).await;
    assert!(favorites::all(&persistence).await.is_err());
}

#[tokio::test]
async fn shared_log_tokens_survive_service_recreation_only_in_keyring_or_session()
 {
    let _host = store::secrets::testing::without_store();
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    store::legacy_user_state::initialize(temp.path(), Some(source)).await;
    let persistence = Persistence::new(temp.path());
    let pool = super::support::empty_pool().await;
    let handle = Handle::with_persistence(pool.clone(), &persistence);
    let logs = SharedLogs::get_all(&handle).await.unwrap();
    assert_eq!(logs[0].1.token, "deletion-secret");
    assert_eq!(logs[0].1.raw, "https://example.invalid/raw");
    let bytes = tokio::fs::read_to_string(shared_logs_path(temp.path()))
        .await
        .unwrap();
    assert!(!bytes.contains("deletion-secret"));
    let mut record = logs[0].1.clone();
    record.token = "replacement-secret".into();
    record.created_at = 999;
    SharedLogs::set(&handle, &"log".into(), Some(&record))
        .await
        .unwrap();
    let reopened = Handle::with_persistence(pool, &persistence);
    let logs = SharedLogs::get_all(&reopened).await.unwrap();
    assert_eq!(logs[0].1.token, "replacement-secret");
    assert_eq!(logs[0].1.created_at, 789);
    SharedLogs::set(&reopened, &"log".into(), None)
        .await
        .unwrap();
    assert!(SharedLogs::get_all(&handle).await.unwrap().is_empty());
    let bytes = tokio::fs::read_to_string(shared_logs_path(temp.path()))
        .await
        .unwrap();
    assert!(!bytes.contains("replacement-secret"));
}
fn shared_logs_path(root: &std::path::Path) -> std::path::PathBuf {
    root.join("data/user-state/shared_logs.json")
}

#[tokio::test]
async fn migrated_daily_totals_are_imported_once_and_survive_core_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("app.db");
    let mut legacy = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE instance_daily_playtime(played_on TEXT,instance_id TEXT,instance_name TEXT,played_seconds INTEGER,session_count INTEGER);INSERT INTO instance_daily_playtime VALUES('2026-11-15','instance','Example',30,1);").execute(&mut legacy).await.unwrap();
    legacy.close().await.unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    store::records_db::initialize(temp.path(), Some(path.clone()), Some(&core))
        .await
        .unwrap();
    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('instance','path','installed','none','Example',0,0)").execute(&core).await.unwrap();
    let day = "2026-11-15".parse().unwrap();
    crate::state::instances::adapters::sqlite::playtime::upsert_daily_playtime(
        "instance", "Example", day, 5, 0, &core,
    )
    .await
    .unwrap();
    store::records_db::initialize(temp.path(), Some(path), Some(&core))
        .await
        .unwrap();
    crate::state::instances::adapters::sqlite::instance_rows::delete_instance_by_id("instance",&core).await.unwrap();
    let totals=crate::state::instances::adapters::sqlite::playtime::get_daily_playtime(day,day,&core).await.unwrap();
    assert_eq!(totals[0].played_seconds, 35);
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM pragma_database_list")
            .fetch_all(&core)
            .await
            .unwrap();
    assert!(!names.iter().any(|name| name == "records"));
    core.close().await;
}

#[tokio::test]
async fn corrupted_records_database_does_not_block_core_loading() {
    let temp = tempfile::tempdir().unwrap();
    tokio::fs::create_dir_all(temp.path().join("databases"))
        .await
        .unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('instance','path','installed','none','Example',0,0)").execute(&core).await.unwrap();
    tokio::fs::write(temp.path().join("data/records.db"), b"broken records")
        .await
        .unwrap();
    assert!(store::records_db::pool_for(&core).await.is_none());
    assert!(crate::state::instances::adapters::sqlite::instance_rows::get_instance_by_id("instance",&core).await.unwrap().is_some());
    assert_eq!(
        tokio::fs::read(temp.path().join("data/records.db"))
            .await
            .unwrap(),
        b"broken records"
    );
    core.close().await;
}

#[tokio::test]
async fn every_instance_reader_uses_pins_and_unpin_never_revives_the_old_column()
 {
    let temp = tempfile::tempdir().unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    let persistence = Persistence::new(temp.path());
    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified,pinned_at) VALUES('instance','path','installed','none','Example',0,0,123)").execute(&core).await.unwrap();
    sqlx::query("INSERT INTO instance_content_sets(id,instance_id,name,source_kind,status,game_version,loader,created,modified) VALUES('set','instance','Local','local','available','1.20.1','vanilla',0,0)").execute(&core).await.unwrap();
    sqlx::query(
        "UPDATE instances SET applied_content_set_id='set' WHERE id='instance'",
    )
    .execute(&core)
    .await
    .unwrap();
    use crate::state::instances::adapters::sqlite::instance_rows as rows;
    rows::set_pinned("instance", Some(456), 0, &core)
        .await
        .unwrap();
    assert_eq!(
        rows::get_instance_by_id("instance", &core)
            .await
            .unwrap()
            .unwrap()
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::get_instance_by_path("path", &core)
            .await
            .unwrap()
            .unwrap()
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::get_instance_metadata_by_id("instance", &core)
            .await
            .unwrap()
            .unwrap()
            .instance
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::get_instance_metadata_many(&["instance"], &core)
            .await
            .unwrap()[0]
            .instance
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::list_instance_metadata(&core).await.unwrap()[0]
            .instance
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::get_instance_launch_context("instance", &core)
            .await
            .unwrap()
            .unwrap()
            .instance
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    assert_eq!(
        rows::list_instances(&core).await.unwrap()[0]
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    rows::set_pinned("instance", None, 0, &core).await.unwrap();
    assert!(
        rows::get_instance_by_id("instance", &core)
            .await
            .unwrap()
            .unwrap()
            .pinned_at
            .is_none()
    );
    assert!(favorites::pin_values(&persistence).await.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT pinned_at FROM instances WHERE id='instance'"
        )
        .fetch_one(&core)
        .await
        .unwrap(),
        Some(123)
    );
    core.close().await;
}

#[tokio::test]
async fn shared_log_cannot_read_or_delete_an_account_credential_reference() {
    let host = store::secrets::testing::with_store();
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    store::legacy_user_state::initialize(temp.path(), Some(source)).await;
    let persistence = Persistence::new(temp.path());
    let pool = super::support::empty_pool().await;
    let handle = Handle::with_persistence(pool, &persistence);
    let account_key =
        store::secrets::key(&["microsoft:account", "access_token"]);
    store::secrets::access()
        .write(&account_key, Some("account-secret"))
        .unwrap();
    let foreign = account_key.clone();
    user_state::shared_logs::domain(&persistence)
        .update(move |entries| {
            entries["log"]["token_ref"] = json!(foreign);
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        SharedLogs::get_all(&handle).await.unwrap()[0]
            .1
            .token
            .is_empty()
    );
    assert!(SharedLogs::set(&handle, &"log".into(), None).await.is_err());
    assert_eq!(
        store::secrets::access()
            .read(&account_key)
            .unwrap()
            .as_deref(),
        Some("account-secret")
    );
    assert!(host.store().is_some());
}

#[tokio::test]
async fn fresh_root_creates_core_before_optional_records_and_backups_include_documents()
 {
    let temp = tempfile::tempdir().unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    assert!(temp.path().join("data/system.db").is_file());
    assert!(temp.path().join("data/records.db").is_file());
    let destination =
        store::databases::backup(&core, temp.path()).await.unwrap();
    assert!(destination.join("records.db").is_file());
    for path in [
        "data/user-state/worlds.json",
        "data/user-state/joins.json",
        "data/user-state/resource_favorites.json",
        "data/user-state/instance_pins.json",
        "data/user-state/shared_logs.json",
        "data/persistence/records.json",
    ] {
        assert!(destination.join(path).is_file(), "{path}");
    }
    core.close().await;
}

#[tokio::test]
async fn existing_daily_authority_does_not_reimport_a_deleted_row_before_descriptor_retry()
 {
    let temp = tempfile::tempdir().unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    core.close().await;
    // Simulate the interval after committed SQL handover but before its JSON
    // receipt publication; the existing table remains the authority.
    tokio::fs::remove_file(temp.path().join("data/persistence/records.json"))
        .await
        .unwrap();
    tokio::fs::remove_file(
        temp.path()
            .join("data/persistence/imports/daily-playtime.json"),
    )
    .await
    .unwrap();
    let source = temp.path().join("old.db");
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&source)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TABLE instance_daily_playtime(played_on TEXT,instance_id TEXT,instance_name TEXT,played_seconds INTEGER,session_count INTEGER);INSERT INTO instance_daily_playtime VALUES('2026-11-15','deleted','Old',100,1);").execute(&mut db).await.unwrap();
    db.close().await.unwrap();
    store::records_db::initialize(temp.path(), Some(source), None)
        .await
        .unwrap();
    let mut current = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(temp.path().join("data/records.db")),
    )
    .await
    .unwrap();
    let rows:i64=sqlx::query_scalar("SELECT COUNT(*) FROM instance_daily_playtime WHERE instance_id='deleted'").fetch_one(&mut current).await.unwrap();
    assert_eq!(rows, 0);
    current.close().await.unwrap();
}

#[tokio::test]
async fn failed_user_document_import_retries_retained_snapshot_instead_of_daily_only_store()
 {
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    // A non-file target makes the first publication fail before a receipt.
    tokio::fs::create_dir_all(temp.path().join("data/user-state/worlds.json"))
        .await
        .unwrap();
    store::legacy_user_state::initialize(temp.path(), Some(source.clone()))
        .await;
    assert!(
        !temp
            .path()
            .join("data/persistence/imports/worlds.json")
            .exists()
    );
    tokio::fs::remove_dir(temp.path().join("data/user-state/worlds.json"))
        .await
        .unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    // Opening retries before this fixture retains the migrated source. Remove
    // that deliberate empty first-install authority to model the pending task.
    for relative in [
        "data/user-state/worlds.json",
        "data/persistence/imports/worlds.json",
    ] {
        let _ = tokio::fs::remove_file(temp.path().join(relative)).await;
    }
    tokio::fs::copy(source, temp.path().join("data/persistence/legacy-app.db"))
        .await
        .unwrap();
    store::legacy_user_state::initialize(temp.path(), None).await;
    let choices = worlds::get_all(&Persistence::new(temp.path()), "instance")
        .await
        .unwrap();
    assert_eq!(choices.len(), 1);
    core.close().await;
}

#[tokio::test]
async fn missing_existing_records_format_row_is_not_seeded_as_a_known_version()
{
    let temp = tempfile::tempdir().unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    core.close().await;
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(temp.path().join("data/records.db")),
    )
    .await
    .unwrap();
    sqlx::query("DELETE FROM axolotl_records_format")
        .execute(&mut db)
        .await
        .unwrap();
    db.close().await.unwrap();
    for relative in [
        "data/persistence/records.json",
        "data/persistence/imports/daily-playtime.json",
    ] {
        tokio::fs::remove_file(temp.path().join(relative))
            .await
            .unwrap();
    }
    assert!(
        store::records_db::initialize(temp.path(), None, None)
            .await
            .is_err()
    );
    let mut db = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(temp.path().join("data/records.db")),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM axolotl_records_format"
        )
        .fetch_one(&mut db)
        .await
        .unwrap(),
        0
    );
    db.close().await.unwrap();
}

#[tokio::test]
async fn interrupted_core_handover_uses_retained_snapshot_before_empty_baseline_domains()
 {
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    let core = store::databases::open(temp.path()).await.unwrap();
    for name in [
        "worlds",
        "joins",
        "shared_logs",
        "resource_favorites",
        "instance_pins",
    ] {
        for relative in [
            format!("data/user-state/{name}.json"),
            format!("data/persistence/imports/{name}.json"),
        ] {
            tokio::fs::remove_file(temp.path().join(relative))
                .await
                .unwrap();
        }
    }
    tokio::fs::copy(source, temp.path().join("data/persistence/legacy-app.db"))
        .await
        .unwrap();
    store::legacy_user_state::initialize(temp.path(), None).await;
    let persistence = Persistence::new(temp.path());
    assert_eq!(favorites::all(&persistence).await.unwrap().len(), 1);
    assert_eq!(
        favorites::pin_values(&persistence).await["instance"].timestamp(),
        456
    );
    assert_eq!(
        worlds::get_all(&persistence, "instance")
            .await
            .unwrap()
            .len(),
        1
    );
    let logs = user_state::shared_logs::domain(&persistence)
        .read()
        .await
        .unwrap();
    assert_eq!(logs["entries"].as_object().unwrap().len(), 1);
    core.close().await;
}

#[tokio::test]
async fn malformed_deprecated_sql_pin_cannot_block_document_pin_read() {
    let temp = tempfile::tempdir().unwrap();
    let core = store::databases::open(temp.path()).await.unwrap();
    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified,pinned_at) VALUES('instance','path','installed','none','Example',0,0,'future-value')").execute(&core).await.unwrap();
    use crate::state::instances::adapters::sqlite::instance_rows as rows;
    rows::set_pinned("instance", Some(456), 0, &core)
        .await
        .unwrap();
    assert_eq!(
        rows::get_instance_by_id("instance", &core)
            .await
            .unwrap()
            .unwrap()
            .pinned_at
            .unwrap()
            .timestamp(),
        456
    );
    core.close().await;
}

#[tokio::test]
async fn completed_core_uses_retained_snapshot_for_each_missing_legacy_table() {
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    let core = store::databases::open(temp.path()).await.unwrap();
    sqlx::query("INSERT INTO main.database_imports VALUES('app.db',1)")
        .execute(&core)
        .await
        .unwrap();
    for relative in [
        "data/user-state/shared_logs.json",
        "data/persistence/imports/shared_logs.json",
    ] {
        tokio::fs::remove_file(temp.path().join(relative))
            .await
            .unwrap();
    }
    let mut records = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(temp.path().join("data/records.db")),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE attached_world_data(instance_id TEXT,world_type TEXT,world_id TEXT,display_status TEXT,project_id TEXT,content_kind TEXT)").execute(&mut records).await.unwrap();
    records.close().await.unwrap();
    tokio::fs::copy(source, temp.path().join("data/persistence/legacy-app.db"))
        .await
        .unwrap();
    store::legacy_user_state::initialize(temp.path(), None).await;
    let data = user_state::shared_logs::domain(&Persistence::new(temp.path()))
        .read()
        .await
        .unwrap();
    assert_eq!(data["entries"].as_object().unwrap().len(), 1);
    core.close().await;
}

#[tokio::test]
async fn grouped_favorite_removal_ignores_future_records_without_losing_supported_removals()
 {
    let temp = tempfile::tempdir().unwrap();
    let source = snapshot(temp.path()).await;
    store::legacy_user_state::initialize(temp.path(), Some(source)).await;
    let persistence = Persistence::new(temp.path());
    favorites::domain(&persistence).update(|entries|{
        entries.insert(user_state::key(&["modrinth","future"]),json!({"source":"modrinth","project_id":"future","content_type":"future-type","saved_at":1}));Ok(())
    }).await.unwrap();
    favorites::remove(
        &persistence,
        vec![
            ("modrinth".into(), "project".into()),
            ("modrinth".into(), "future".into()),
        ],
    )
    .await
    .unwrap();
    let entries = favorites::all(&persistence).await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].content_type, "future-type");
}
