//! SQL is only the read-only old sync importer and coordinator decision record.
use super::Collection;
use crate::state::{
    layout,
    persistence::{Persistence, import::Import},
    sync_state::{self, manifest},
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::{
    Column, Connection, Row, SqliteConnection, SqlitePool, TypeInfo, ValueRef,
};
use std::path::{Path, PathBuf};
pub(crate) async fn configured_root(root: &Path) -> crate::Result<PathBuf> {
    let pool = SqlitePool::connect_lazy_with(
        sqlx::sqlite::SqliteConnectOptions::new().in_memory(true),
    );
    let documents = crate::state::documents::Documents::at(
        root.join(layout::SETTINGS),
        &pool,
    )
    .with_config_root(crate::state::documents::config_root());
    crate::state::documents::settings::content_root_before_state(
        root, &documents,
    )
    .await
}
pub(crate) async fn initialize(root: &Path, snapshot: Option<PathBuf>) {
    if let Err(error) = initialize_inner(root, snapshot).await {
        tracing::warn!(%error,"Independent sync unavailable; instances remain available");
    }
}
async fn initialize_inner(
    root: &Path,
    snapshot: Option<PathBuf>,
) -> crate::Result<()> {
    let persistence = Persistence::new(root);
    let shared = sync_state::root(&persistence).await?;
    let target = persistence.document(
        layout::SYNC_AUTHORITY,
        "axolotl-sync-authority",
        1,
    )?;
    let source_root = root.to_owned();
    let initial_snapshot = snapshot.clone();
    let result = Import::new(&persistence,"sync",target)?.run(move||async move{
  if initial_snapshot.is_none() && !source_root.join("databases/sync.db").is_file() && ["app.db","release/app.db","beta/app.db"].iter().any(|path|source_root.join(path).is_file()) && !source_root.join(layout::IMPORT_SNAPSHOT).is_file(){return Err(crate::ErrorKind::FSError("Shared sync import awaits migrated source".into()).into());}
  let read=manifest::document(&shared).read().await?;
  if read.availability==crate::state::persistence::status::Availability::Missing{
   let mut collections=manifest::Collections::new();let mut opaque=false;
   if let Some(mut connection)=super::legacy_documents::read_source_for(&source_root,"sync","synced_server_state",snapshot).await?{
    opaque |= capture_raw(&mut connection,&mut collections).await?;
    macro_rules! capture {
        ($module:ident,$collection:ident) => {{
            let name = stringify!($module);
            let table = match name {
                "game_option_pack_bases" => "instance_game_option_pack_bases",
                "game_option_update_state" => "instance_game_option_update_state",
                other => other,
            };
            if !super::legacy_documents::columns(&mut connection, table).await?.is_empty() {
                let captured = <super::$module::$collection as Collection>::read_all(&mut connection).await;
                match captured.and_then(|records| capture_records::<super::$module::$collection>(&mut collections, name, records)) {
                    Ok(()) => {},
                    Err(error) => {
                        opaque = true;
                        tracing::warn!(%error, table=name, "Unreadable legacy sync records retained in raw attachment");
                    }
                }
            }
        }};
    }
    capture!(synced_servers,SyncedServers);capture!(synced_server_state,SyncedServerState);capture!(synced_game_option_state,SyncedGameOptionState);capture!(synced_game_option_values,SyncedGameOptionValues);capture!(synced_game_option_preferences,SyncedGameOptionPreferences);capture!(synced_hotbar_state,SyncedHotbarState);capture!(synced_pack_catalog,SyncedPackCatalog);capture!(synced_pack_instances,SyncedPackInstances);capture!(game_option_locale_origins,GameOptionLocaleOrigins);capture!(game_option_pack_bases,GameOptionPackBases);capture!(game_option_update_state,GameOptionUpdateState);capture!(instance_sync_checkpoints,InstanceSyncCheckpoints);
    if let Err(error)=capture_servers(&mut connection,&mut collections).await{opaque=true;tracing::warn!(%error,"Unreadable legacy server records retained in raw attachment");}
    opaque |= !verify_counts(&collections);
    connection.close().await?;
   }
   let history=shared.join("command_history.txt");
   let legacy_shared=configured_root(&source_root).await?.join("synced-options");
   copy_native_sources(&legacy_shared, &shared).await?;
   copy_pack_files(&legacy_shared, &shared).await?;
   if history.is_file(){collections.entry("native_text".into()).or_default().insert("command_history".into(),json!({"record":{"document":manifest::read_bounded(&history,16<<20).await?}}));}
   manifest::initial(&shared,collections,opaque).await?;
  }else{manifest::load(&shared).await?;}
  Ok(Map::from_iter([("root".into(),json!(shared)),("contract".into(),json!(1))]))
 },|snapshot|{
  let data=snapshot.data().ok_or_else(||crate::ErrorKind::InputError("Missing sync authority".into()))?;
  if data.get("contract").and_then(Value::as_u64)!=Some(1)||!data.get("root").is_some_and(Value::is_string){return Err(crate::ErrorKind::InputError("Unsupported sync authority".into()).into());}Ok(())
 }).await?;
    if !matches!(
        result.availability,
        crate::state::persistence::status::Availability::Ready
            | crate::state::persistence::status::Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Shared sync handover is unavailable or unsupported".into(),
        )
        .into());
    }
    Ok(())
}
async fn copy_native_sources(
    source: &Path,
    destination: &Path,
) -> crate::Result<()> {
    for name in [
        "options.txt",
        "command_history.txt",
        "hotbar.nbt",
        "servers.dat",
    ] {
        let path = source.join(name);
        let target = destination.join(name);
        if tokio::fs::try_exists(&target).await? {
            continue;
        }
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if metadata.is_file() => {
                crate::state::persistence::document::write_blob(
                    &target,
                    manifest::read_bounded(&path, 16 << 20).await?,
                )
                .await?;
            }
            Ok(_) => {
                return Err(crate::ErrorKind::FSError(format!(
                    "Shared sync source {} is not a regular file",
                    path.display()
                ))
                .into());
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

pub(super) async fn copy_pack_files(
    source: &Path,
    destination: &Path,
) -> crate::Result<()> {
    let packs = source.join("packs/files");
    let mut entries = match tokio::fs::read_dir(&packs).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let target_packs = destination.join("packs/files");
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        if !entry.file_type().await?.is_file()
            || !name.to_str().is_some_and(|name| {
                name.len() == 40
                    && name.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            continue;
        }
        let target = target_packs.join(name);
        if tokio::fs::try_exists(&target).await? {
            continue;
        }
        tokio::fs::create_dir_all(&target_packs).await?;
        let staged =
            target.with_extension(format!("{}.next", uuid::Uuid::new_v4()));
        tokio::fs::copy(entry.path(), &staged).await?;
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(&staged)
            .await?
            .sync_all()
            .await?;
        tokio::fs::rename(staged, target).await?;
    }
    Ok(())
}

/// Preserve every source cell, including columns and enum values this binary
/// cannot interpret. Blob cells are immutable binary objects after preparation.
async fn capture_raw(
    connection: &mut SqliteConnection,
    collections: &mut manifest::Collections,
) -> crate::Result<bool> {
    let baseline: Value =
        serde_json::from_str(include_str!("database_baseline.json"))?;
    let mut opaque = false;
    let mut total_bytes = 0usize;
    for table in baseline["tables"]
        .as_array()
        .expect("frozen tables")
        .iter()
        .filter(|table| table["domain"] == "sync")
    {
        let name = table["name"].as_str().expect("table name");
        let columns =
            super::legacy_documents::columns(connection, name).await?;
        if columns.is_empty() {
            continue;
        }
        let known = table["columns"].as_array().expect("frozen columns");
        opaque |= columns.iter().any(|column| {
            !known.iter().any(|value| value.as_str() == Some(column))
        });
        let query = format!("SELECT * FROM \"{}\"", name.replace('"', "\"\""));
        let mut rows = Map::new();
        let mut source_rows = sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
            .fetch(&mut *connection);
        use futures::TryStreamExt;
        while let Some(row) = source_rows.try_next().await? {
            let mut fields = Map::new();
            for (position, column) in row.columns().iter().enumerate() {
                let raw = row.try_get_raw(position)?;
                let value = if raw.is_null() {
                    Value::Null
                } else {
                    match raw.type_info().name() {
                        "INTEGER" => json!({"type":"integer","value":row.try_get::<i64,_>(position)?}),
                        "REAL" => json!({"type":"real","value":row.try_get::<f64,_>(position)?}),
                        "TEXT" => json!({"type":"text","value":row.try_get::<String,_>(position)?}),
                        "BLOB" => json!({"type":"blob","document":row.try_get::<Vec<u8>,_>(position)?}),
                        _ => return Err(crate::ErrorKind::InputError("Unrecognized legacy SQLite value retained at source".into()).into()),
                    }
                };
                fields.insert(column.name().into(), value);
            }
            let value = json!({"cells":fields});
            total_bytes = total_bytes
                .checked_add(serde_json::to_vec(&value)?.len())
                .filter(|size| *size <= 64 * 1024 * 1024)
                .ok_or_else(|| crate::ErrorKind::InputError(
                    "Legacy sync snapshot exceeds the supported import size".into(),
                ))?;
            rows.insert(rows.len().to_string(), value);
        }
        collections.insert(format!("$legacy/{name}"), rows);
    }
    if !super::legacy_documents::columns(connection, "extension_records")
        .await?
        .is_empty()
    {
        let rows: Vec<(String, String, String, Vec<u8>)> = sqlx::query_as(
            "SELECT namespace,id,format,payload FROM extension_records",
        )
        .fetch_all(&mut *connection)
        .await?;
        if !rows.is_empty() {
            opaque = true;
            collections.insert("$legacy/extensions".into(),rows.into_iter().enumerate().map(|(index,row)|(index.to_string(),json!({"namespace":row.0,"id":row.1,"format":row.2,"document":row.3}))).collect());
        }
    }
    Ok(opaque)
}
fn verify_counts(collections: &manifest::Collections) -> bool {
    collections
        .iter()
        .filter_map(|(name, rows)| {
            name.strip_prefix("$legacy/").map(|name| (name, rows.len()))
        })
        .all(|(name, count)| {
            let logical = match name {
                "instance_game_option_pack_bases" => "game_option_pack_bases",
                "instance_game_option_update_state" => {
                    "game_option_update_state"
                }
                other => other,
            };
            logical == "extensions"
                || collections.get(logical).map_or(0, Map::len) == count
        })
}
fn capture_records<C: Collection>(
    collections: &mut manifest::Collections,
    name: &str,
    records: Vec<(C::Key, C::Record)>,
) -> crate::Result<()>
where
    C::Key: Serialize,
    C::Record: Serialize,
{
    let mut entries = Map::new();
    for (key, record) in records {
        let identity = serde_json::to_value(&key)?;
        entries.insert(
            sync_state::key(&identity)?,
            json!({"key":identity,"record":record}),
        );
    }
    collections.insert(name.into(), entries);
    Ok(())
}
async fn capture_servers(
    connection: &mut SqliteConnection,
    collections: &mut manifest::Collections,
) -> crate::Result<()> {
    use crate::state::sync_state::servers::*;
    if !super::legacy_documents::columns(connection, "instance_servers")
        .await?
        .is_empty()
    {
        let mut entries = Map::new();
        for row in sqlx::query_as::<_,(String,String,String,Option<String>,Vec<u8>,i64)>("SELECT instance_id,id,source,excluded_synced_server_id,nbt,position FROM instance_servers").fetch_all(&mut *connection).await?{let key=LocalKey{instance_id:row.0,id:row.1};let record=LocalRecord{source:row.2,excluded_synced_server_id:row.3,nbt:row.4,position:row.5};let raw=serde_json::to_value(&key)?;entries.insert(sync_state::key(&raw)?,json!({"key":raw,"record":record}));}
        collections.insert("instance_servers".into(), entries);
    }
    if !super::legacy_documents::columns(
        connection,
        "instance_server_projection_entries",
    )
    .await?
    .is_empty()
    {
        let mut entries = Map::new();
        for row in sqlx::query_as::<_,(String,String,String,Vec<u8>,i64)>("SELECT instance_id,owner,server_id,nbt,position FROM instance_server_projection_entries").fetch_all(&mut *connection).await?{let key=ProjectionKey{instance_id:row.0,owner:row.1,server_id:row.2};let record=ProjectionRecord{nbt:row.3,position:row.4};let raw=serde_json::to_value(&key)?;entries.insert(sync_state::key(&raw)?,json!({"key":raw,"record":record}));}
        collections
            .insert("instance_server_projection_entries".into(), entries);
    }
    if !super::legacy_documents::columns(
        connection,
        "instance_server_pack_state",
    )
    .await?
    .is_empty()
    {
        let mut entries = Map::new();
        for (id, version) in sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT instance_id,version_id FROM instance_server_pack_state",
        )
        .fetch_all(&mut *connection)
        .await?
        {
            let raw = json!(id);
            entries.insert(
                sync_state::key(&raw)?,
                json!({"key":raw,"record":{"version_id":version}}),
            );
        }
        collections.insert("instance_server_pack_state".into(), entries);
    }
    Ok(())
}
pub(crate) async fn install(
    connection: &mut SqliteConnection,
) -> Result<(), sqlx::Error> {
    sqlx::query("CREATE TABLE IF NOT EXISTS main.sync_decisions(singleton INTEGER PRIMARY KEY CHECK(singleton=1),token TEXT NOT NULL)").execute(connection).await?;
    Ok(())
}
pub(crate) async fn decision(
    connection: &mut SqliteConnection,
) -> crate::Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT token FROM main.sync_decisions WHERE singleton=1",
    )
    .fetch_optional(connection)
    .await?)
}

pub(crate) async fn read_collections(
    handle: &super::Handle,
) -> crate::Result<manifest::Collections> {
    super::recovery_mode::ensure_writable(handle.pool())?;
    let persistence = handle.persistence();
    super::recovery_mode::ensure_path_writable(persistence.root())?;
    let root = authority(&persistence).await?;
    let mut connection = handle.pool().acquire().await?;
    // Publication also holds the file lock through the SQL decision and head
    // update. Sample the committed token under that lock using autocommit so
    // a reader neither reserves the writer nor waits with a SQL read lock.
    let _lock = sync_state::journal::initial_lock(&root).await?;
    let committed = decision(&mut connection).await?;
    drop(connection);
    if let Some(token) = committed {
        sync_state::journal::replay(&root, &token).await?;
    }
    let (_, collections) = manifest::load(&root).await?;
    Ok(collections)
}

pub(crate) async fn decide(
    connection: &mut SqliteConnection,
    token: &str,
) -> crate::Result<()> {
    sqlx::query("INSERT INTO main.sync_decisions VALUES(1,?) ON CONFLICT(singleton) DO UPDATE SET token=excluded.token").bind(token).execute(connection).await?;
    Ok(())
}
pub(crate) async fn clear(pool: &SqlitePool, token: &str) -> crate::Result<()> {
    sqlx::query(
        "DELETE FROM main.sync_decisions WHERE singleton=1 AND token=?",
    )
    .bind(token)
    .execute(pool)
    .await?;
    Ok(())
}

pub(crate) async fn validate_parents(
    connection: &mut SqliteConnection,
    collection: &str,
    key: &Value,
) -> crate::Result<()> {
    let instance = match collection {
        "instance_servers"
        | "instance_server_projection_entries"
        | "instance_sync_checkpoints"
        | "synced_pack_instances"
        | "game_option_pack_bases"
        | "game_option_update_state" => {
            key.get("instance_id").and_then(Value::as_str)
        }
        "instance_server_pack_state" => key.as_str(),
        _ => return Ok(()),
    }
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Sync record is missing its instance identity".into(),
        )
    })?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM instances WHERE id=?)")
            .bind(instance)
            .fetch_one(&mut *connection)
            .await?;
    if !exists {
        return Err(crate::ErrorKind::InputError(
            "This instance has been deleted".into(),
        )
        .into());
    }
    Ok(())
}

/// Resolve a handed-over root only if it still matches trusted launcher storage.
/// A missing head after the receipt is an availability fault, never permission
/// to recreate an empty authority or fall back to a retired database.
pub(crate) async fn authority(
    persistence: &Persistence,
) -> crate::Result<std::path::PathBuf> {
    let read = persistence
        .document(layout::SYNC_AUTHORITY, "axolotl-sync-authority", 1)?
        .read()
        .await?;
    if !matches!(
        read.availability,
        crate::state::persistence::status::Availability::Ready
            | crate::state::persistence::status::Availability::Recovered
    ) {
        return Err(crate::ErrorKind::FSError(
            "Sync authority descriptor unavailable".into(),
        )
        .into());
    }
    let snapshot = read.snapshot.expect("descriptor");
    let data = snapshot.data().expect("descriptor payload");
    let configured = sync_state::root(persistence).await?;
    verify_authority(data, &configured).await?;
    Ok(configured)
}

async fn verify_authority(
    data: &Map<String, Value>,
    expected: &Path,
) -> crate::Result<()> {
    if data.get("contract").and_then(Value::as_u64) != Some(1)
        || data.get("protocol").is_some()
        || ["schema_version", "schemaVersion"].iter().any(|key| {
            data.get(*key)
                .is_some_and(|value| value.as_u64() != Some(1))
        })
    {
        return Err(crate::ErrorKind::InputError(
            "Unsupported sync authority descriptor retained".into(),
        )
        .into());
    }
    let stored = data.get("root").and_then(Value::as_str).ok_or_else(|| {
        crate::ErrorKind::InputError("Sync authority root missing".into())
    })?;
    let stored = std::path::Path::new(stored);
    if stored != expected && !same_sync_directory(stored, expected).await {
        return Err(crate::ErrorKind::FSError(
            "Sync storage moved without its authority; original state retained"
                .into(),
        )
        .into());
    }
    Ok(())
}

pub(crate) async fn cleanup_history(
    settings_root: &Path,
    copied_from: Option<&Path>,
    apply: bool,
    limit: Option<usize>,
    progress: impl FnMut(&sync_state::gc::SyncCleanupReport),
    failure: impl FnMut(&str, &crate::Error),
) -> crate::Result<sync_state::gc::SyncCleanupReport> {
    use sync_state::gc::{checked_path, read_envelope, require_path};
    let settings_root = tokio::fs::canonicalize(settings_root).await?;
    let data = layout::data(&settings_root);
    require_path(&data, true).await?;
    let persistence = Persistence::new(settings_root);
    let root = sync_state::root(&persistence).await?;
    require_path(&root, true).await?;
    let database = data.join("system.db");
    require_path(&database, false).await?;
    checked_path(&data.join("system.db.ownership.lock"), false).await?;
    let _ownership =
        crate::state::db::acquire_domain_ownership(&database).await?;
    for suffix in ["-journal", "-wal", "-shm", ".replacement.json"] {
        if checked_path(&data.join(format!("system.db{suffix}")), false)
            .await?
            .is_some()
        {
            return Err(crate::ErrorKind::InputError(
                "Recover the coordinator with the launcher before cleaning sync history".into(),
            ).into());
        }
    }
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(database)
        .create_if_missing(false)
        .busy_timeout(std::time::Duration::ZERO);
    let mut connection = SqliteConnection::connect_with(&options).await?;
    // Reserve the same coordinator writer as Atom before the sync file lock.
    // No sync prepare/decision can race the recovery-root snapshot below.
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut connection)
        .await?;
    let token = decision(&mut connection).await?;
    let _lock = sync_state::gc::lock(&root).await?;
    let authority = read_envelope(
        &persistence.root().join(layout::SYNC_AUTHORITY),
        "axolotl-sync-authority",
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Sync authority descriptor is missing".into(),
        )
    })?;
    let authority = authority.as_object().expect("authority payload");
    if authority.len() != 2 {
        return Err(crate::ErrorKind::InputError(
            "Unknown sync authority metadata".into(),
        )
        .into());
    }
    let expected = copied_from.map(|path| path.join(layout::SYNC_ROOT));
    verify_authority(authority, expected.as_deref().unwrap_or(&root)).await?;
    sync_state::gc::scan(
        &root,
        token.as_deref(),
        apply,
        limit,
        progress,
        failure,
    )
    .await
}

async fn same_sync_directory(
    stored: &std::path::Path,
    current: &std::path::Path,
) -> bool {
    #[cfg(windows)]
    {
        // The coordinator opens SQLite with an extended path, while an
        // existing sync descriptor can name that same directory without it.
        match tokio::try_join!(
            tokio::fs::canonicalize(stored),
            tokio::fs::canonicalize(current),
        ) {
            Ok((stored, current)) => stored == current,
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (stored, current);
        false
    }
}

pub(crate) async fn backup_in(
    connection: &mut SqliteConnection,
    persistence: &Persistence,
    destination: &Path,
) -> crate::Result<()> {
    let shared = authority(persistence).await?;
    let committed = decision(connection).await?;
    let transaction =
        sync_state::journal::Transaction::begin(shared, committed.clone())
            .await?;
    manifest::backup(&transaction.root, destination, committed.as_deref())
        .await?;
    copy_native_sources(&transaction.root, destination).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store::synced_game_option_state::{
        GameOptionStateRecord, Singleton, SyncedGameOptionState,
    };

    #[tokio::test]
    async fn history_cleanup_requires_ownership_and_honors_committed_decision()
    {
        let root = tempfile::tempdir().unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        let persistence = Persistence::new(root.path());
        let shared = authority(&persistence).await.unwrap();
        let mut sql = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let mut sync =
            sync_state::journal::Transaction::begin(shared.clone(), None)
                .await
                .unwrap();
        sync.changed = true;
        let token = sync.prepare().await.unwrap().unwrap();
        decide(&mut sql, &token).await.unwrap();
        assert!(
            cleanup_history(root.path(), None, false, None, |_| {}, |_, _| {})
                .await
                .is_err()
        );
        sql.commit().await.unwrap();
        pool.close().await;
        drop(pool);
        assert!(
            cleanup_history(root.path(), None, false, None, |_| {}, |_, _| {})
                .await
                .is_err()
        );
        drop(sync);
        let report = cleanup_history(
            root.path(),
            None,
            false,
            None,
            |_| {},
            |_, error| panic!("{error}"),
        )
        .await
        .unwrap();
        assert_eq!(report.retained_for_recovery, 1);
        assert_eq!(manifest::load(&shared).await.unwrap().0.revision, 0);
        assert!(
            shared
                .join(format!("state/journals/{token}.json"))
                .is_file()
        );
    }

    #[tokio::test]
    async fn raw_import_keeps_every_table_cell_and_unknown_column() {
        let pool = crate::state::store::tests::support::migrated_pool().await;
        sqlx::query(
            "ALTER TABLE synced_server_state ADD COLUMN future_hint TEXT",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT OR REPLACE INTO synced_server_state(singleton,revision,future_hint) VALUES(1,7,'retain')")
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT OR REPLACE INTO synced_hotbar_state(singleton,schema_version,revision,nbt) VALUES(1,2,9,?)")
            .bind(vec![0u8, 1, 255]).execute(&pool).await.unwrap();
        let mut source = pool.acquire().await.unwrap();
        let mut collections = manifest::Collections::new();
        assert!(capture_raw(&mut source, &mut collections).await.unwrap());
        let baseline: Value =
            serde_json::from_str(include_str!("database_baseline.json"))
                .unwrap();
        for table in baseline["tables"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|table| table["domain"] == "sync")
        {
            let name = table["name"].as_str().unwrap();
            assert!(collections.contains_key(&format!("$legacy/{name}")));
        }
        assert_eq!(
            collections["$legacy/synced_server_state"]["0"]["cells"]["future_hint"]
                ["value"],
            "retain",
        );
        assert_eq!(
            collections["$legacy/synced_hotbar_state"]["0"]["cells"]["nbt"]["document"],
            json!([0, 1, 255]),
        );
    }

    #[tokio::test]
    async fn storage_root_uses_deployment_then_local_bootstrap() {
        let original = tempfile::tempdir().unwrap();
        let deployment = tempfile::tempdir().unwrap();
        let custom = tempfile::tempdir().unwrap();
        let local = tempfile::tempdir().unwrap();
        let pool = crate::state::store::tests::support::empty_pool().await;
        tokio::fs::create_dir_all(original.path().join("data/settings"))
            .await
            .unwrap();
        tokio::fs::write(deployment.path().join("bootstrap.json"),
            serde_json::to_vec(&json!({"schema_version":1,"data":{"custom_dir":custom.path()}})).unwrap())
            .await.unwrap();
        let documents = crate::state::documents::Documents::at(
            original.path().join("data/settings"),
            &pool,
        )
        .with_config_root(Some(deployment.path().to_owned()));
        assert_eq!(
            crate::state::documents::settings::content_root_before_state(
                original.path(),
                &documents
            )
            .await
            .unwrap(),
            custom.path(),
        );
        tokio::fs::write(
            original.path().join("data/settings/bootstrap.json"),
            serde_json::to_vec(
                &json!({"schema_version":1,"data":{"custom_dir":local.path()}}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(
            crate::state::documents::settings::content_root_before_state(
                original.path(),
                &documents
            )
            .await
            .unwrap(),
            local.path(),
        );
    }

    #[tokio::test]
    async fn committed_settings_and_sync_recover_after_missing_publication() {
        let root = tempfile::tempdir().unwrap();
        let pool = crate::state::store::databases::open(root.path())
            .await
            .unwrap();
        let handle = super::super::Handle::new(pool.clone());
        let shared = authority(&handle.persistence()).await.unwrap();
        let mut sql = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let mut sync = sync_state::journal::Transaction::begin(shared, None)
            .await
            .unwrap();
        let key = serde_json::to_value(Singleton).unwrap();
        sync.collections.entry("synced_game_option_state".into()).or_default()
            .insert(sync_state::key(&key).unwrap(), json!({"key":key,"record":{"revision":12,"catalog_revision":3}}));
        sync.changed = true;
        let token = sync.prepare().await.unwrap().unwrap();
        decide(&mut sql, &token).await.unwrap();
        super::super::configuration::prepare_documents(&mut sql,root.path(),vec![("resources".into(),json!({"schema_version":1,"data":{"preferred_source":"curseforge"}}))]).await.unwrap();
        sql.commit().await.unwrap();
        drop(sync);
        // The sync decision is already committed. A foreground writer may
        // obtain its reservation before a reader gets around to replaying it.
        let held_writer = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        for _ in 0..2 {
            assert_eq!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    SyncedGameOptionState::get(&handle, &Singleton),
                )
                .await
                .expect("reading a committed sync decision must not reserve the writer")
                .unwrap(),
                Some(GameOptionStateRecord {
                    revision: 12,
                    catalog_revision: 3
                }),
            );
        }
        held_writer.rollback().await.unwrap();
        super::super::configuration::flush_committed(&pool)
            .await
            .unwrap();
        let published: Value = serde_json::from_slice(
            &tokio::fs::read(root.path().join("data/settings/resources.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(published["data"]["preferred_source"], "curseforge");
        tokio::fs::write(
            root.path().join("data/settings/resources.json"),
            br#"{"schema_version":1,"data":{"preferred_source":"mcarchive"}}"#,
        )
        .await
        .unwrap();
        SyncedGameOptionState::get(&handle, &Singleton)
            .await
            .unwrap();
        let settings: Value = serde_json::from_slice(
            &tokio::fs::read(root.path().join("data/settings/resources.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(settings["data"]["preferred_source"], "mcarchive");
        assert_eq!(
            manifest::load(&authority(&handle.persistence()).await.unwrap())
                .await
                .unwrap()
                .0
                .revision,
            1
        );
    }

    #[tokio::test]
    async fn corrupted_retired_sync_does_not_block_core_or_revive_after_handover()
     {
        let root = tempfile::tempdir().unwrap();
        let pool = crate::state::store::databases::open(root.path())
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let shared = root.path().join(layout::SYNC_ROOT);
        tokio::fs::remove_file(shared.join("state/head.json"))
            .await
            .unwrap();
        tokio::fs::remove_file(shared.join("state/head.json.previous"))
            .await
            .ok();
        tokio::fs::create_dir(root.path().join("databases"))
            .await
            .unwrap();
        tokio::fs::write(
            root.path().join("databases/sync.db"),
            b"corrupt retired SQLite",
        )
        .await
        .unwrap();
        let pool = crate::state::store::databases::open(root.path())
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM instances")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
        assert!(manifest::load(&shared).await.is_err());
        assert_eq!(
            tokio::fs::read(root.path().join("databases/sync.db"))
                .await
                .unwrap(),
            b"corrupt retired SQLite"
        );
    }
}
