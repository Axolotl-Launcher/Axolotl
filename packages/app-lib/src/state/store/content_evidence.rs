//! Rebuildable evidence. The core owns identities; cache rows only describe
//! the exact file and content-set revision recorded by their guard.
use super::{Handle, derived_db::DerivedDb};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{SqliteConnection, SqlitePool};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, OnceCell};

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow,
)]
pub(crate) struct Guard {
    pub instance_id: String,
    pub file_id: String,
    pub relative_path: String,
    pub sha1: String,
    pub size: i64,
    pub content_set_id: Option<String>,
    pub revision: Option<i64>,
    pub location: String,
}

pub(crate) async fn guards(
    connection: &mut SqliteConnection,
    instance: &str,
) -> crate::Result<HashMap<String, Guard>> {
    let rows = sqlx::query_as::<_, Guard>(
        "SELECT file.instance_id,file.id AS file_id,file.relative_path,file.sha1,file.size,
         instance.applied_content_set_id AS content_set_id,content.revision,
         json_array(instance.path,instance.game_dir_override,instance.linked_launcher,instance.linked_launcher_root,instance.linked_dot_minecraft,instance.linked_version_id,instance.linked_version_json_path,instance.linked_game_dir_mode,content.game_version,content.loader,content.loader_version,instance.update_channel) AS location
         FROM instance_files file JOIN instances instance ON instance.id=file.instance_id
         LEFT JOIN instance_content_sets content ON content.id=instance.applied_content_set_id
         WHERE file.instance_id=?")
        .bind(instance).fetch_all(connection).await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.file_id.clone(), row))
        .collect())
}

pub(crate) async fn entry_files(
    connection: &mut SqliteConnection,
    instance: &str,
) -> crate::Result<HashMap<String, String>> {
    Ok(sqlx::query_as::<_, (String,String)>("SELECT entry.id,entry.file_id FROM instance_content_entries entry JOIN instances instance ON instance.id=entry.instance_id WHERE entry.instance_id=? AND entry.content_set_id=instance.applied_content_set_id AND entry.file_id IS NOT NULL")
        .bind(instance).fetch_all(connection).await?.into_iter().collect())
}

async fn cache(handle: &Handle) -> Option<Arc<DerivedDb>> {
    cache_at(handle.persistence().root().join("databases/derived.db")).await
}

async fn cache_at(path: PathBuf) -> Option<Arc<DerivedDb>> {
    if let Some(state) = crate::State::get_if_initialized()
        && path
            == state
                .directories
                .databases_dir()
                .join(super::derived_db::FILE_NAME)
    {
        return (!state.derived.is_disabled())
            .then(|| Arc::new(state.derived.clone()));
    }
    type Cache = Arc<OnceCell<Arc<DerivedDb>>>;
    static CACHES: OnceLock<Mutex<HashMap<PathBuf, Cache>>> = OnceLock::new();
    let cell = {
        let mut caches = CACHES.get_or_init(Mutex::default).lock().await;
        caches
            .entry(path.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone()
    };
    let database = cell
        .get_or_init(|| async { Arc::new(DerivedDb::open(path).await) })
        .await
        .clone();
    (!database.is_disabled()).then_some(database)
}

pub(crate) async fn current(
    handle: &Handle,
    instance: &str,
) -> crate::Result<HashMap<String, Guard>> {
    let mut connection = handle.pool().acquire().await?;
    guards(&mut connection, instance).await
}

pub(crate) async fn read<T: DeserializeOwned>(
    handle: &Handle,
    kind: &str,
    instance: &str,
    id: &str,
    expected: &Guard,
) -> Option<T> {
    let database = cache(handle).await?;
    let row = sqlx::query_as::<_, (String,String)>("SELECT guard,payload FROM content_evidence WHERE kind=? AND instance_id=? AND record_id=?")
        .bind(kind).bind(instance).bind(id).fetch_optional(database.pool()).await;
    match row {
        Ok(Some((guard, payload)))
            if serde_json::from_str::<Guard>(&guard).ok().as_ref()
                == Some(expected) =>
        {
            let value: serde_json::Value =
                serde_json::from_str(&payload).ok()?;
            if ["schema_version", "schemaVersion", "protocol"]
                .iter()
                .any(|key| value.get(*key).is_some())
            {
                return None;
            }
            serde_json::from_value(value).ok()
        }
        Ok(_) => None,
        Err(error) => {
            tracing::warn!(%error,"Content evidence unavailable; treating as a cache miss");
            None
        }
    }
}

pub(crate) async fn read_scoped<T: DeserializeOwned>(
    handle: &Handle,
    kind: &str,
    instance: &str,
    expected: &HashMap<String, Guard>,
    entries: Option<&HashMap<String, String>>,
) -> HashMap<String, T> {
    let Some(database) = cache(handle).await else {
        return HashMap::new();
    };
    let rows=match sqlx::query_as::<_,(String,String,String)>("SELECT record_id,guard,payload FROM content_evidence WHERE kind=? AND instance_id=?").bind(kind).bind(instance).fetch_all(database.pool()).await {
        Ok(rows)=>rows,Err(error)=>{tracing::warn!(%error,"Content evidence unavailable");return HashMap::new();}
    };
    rows.into_iter()
        .filter_map(|(id, guard, payload)| {
            let file =
                entries.and_then(|entries| entries.get(&id)).unwrap_or(&id);
            let expected = expected.get(file)?;
            let mut saved = serde_json::from_str::<Guard>(&guard).ok()?;
            // Byte fingerprints still require current path, identity, bytes
            // and location; unrelated relationship revisions do not change
            // that physical baseline. Consumers validate native attributes.
            if kind == "fingerprint"
                && matches!((saved.revision, expected.revision),
                    (Some(previous), Some(current)) if previous >= 0 && previous <= current)
            {
                saved.revision = expected.revision;
            }
            if &saved != expected {
                return None;
            }
            let raw: serde_json::Value = serde_json::from_str(&payload).ok()?;
            if ["schema_version", "schemaVersion", "protocol"]
                .iter()
                .any(|key| raw.get(*key).is_some())
            {
                return None;
            }
            Some((id, serde_json::from_value(raw).ok()?))
        })
        .collect()
}

/// Used only after a journaled rename. The caller must validate the new
/// physical file against the saved byte and modification-time baseline.
pub(crate) async fn read_for_transfer<T: DeserializeOwned>(
    handle: &Handle,
    kind: &str,
    instance: &str,
    id: &str,
    expected_new: &Guard,
    original_path: &str,
) -> Option<T> {
    let database = cache(handle).await?;
    let (guard, payload) = sqlx::query_as::<_, (String, String)>(
        "SELECT guard,payload FROM content_evidence WHERE kind=? AND instance_id=? AND record_id=?",
    )
    .bind(kind).bind(instance).bind(id)
    .fetch_optional(database.pool()).await.ok()??;
    let mut saved: Guard = serde_json::from_str(&guard).ok()?;
    if saved.relative_path != original_path
        || !matches!((saved.revision, expected_new.revision),
            (Some(previous), Some(current)) if previous >= 0 && previous <= current)
    {
        return None;
    }
    saved.relative_path = expected_new.relative_path.clone();
    saved.revision = expected_new.revision;
    if &saved != expected_new {
        return None;
    }
    let raw: serde_json::Value = serde_json::from_str(&payload).ok()?;
    if ["schema_version", "schemaVersion", "protocol"]
        .iter()
        .any(|key| raw.get(*key).is_some())
    {
        return None;
    }
    serde_json::from_value(raw).ok()
}

pub(crate) struct Write {
    pub kind: &'static str,
    pub id: String,
    pub guard: Guard,
    pub payload: String,
}
pub(crate) async fn write_batch(
    handle: &Handle,
    instance: &str,
    records: Vec<Write>,
) {
    if let Err(error) = try_write_batch(handle, instance, records).await {
        tracing::warn!(%error, "Optional evidence batch was not persisted");
    }
}

/// A refresh reports persistence failure to its own retry queue. Scans use the
/// best-effort wrapper because losing disposable evidence must not block them.
pub(crate) async fn try_write_batch(
    handle: &Handle,
    instance: &str,
    records: Vec<Write>,
) -> crate::Result<()> {
    if records.is_empty() {
        return Ok(());
    }
    let current = current(handle, instance).await?;
    let database = cache(handle).await.ok_or_else(|| {
        crate::ErrorKind::FSError(
            "Optional content evidence is unavailable".into(),
        )
    })?;
    let mut tx = database.pool().begin().await?;
    for record in records {
        if current.get(&record.guard.file_id) != Some(&record.guard) {
            return Err(crate::ErrorKind::InputError(
                "Content changed before refresh evidence publication".into(),
            )
            .into());
        }
        let guard = serde_json::to_string(&record.guard)?;
        sqlx::query("INSERT INTO content_evidence(kind,instance_id,record_id,guard,payload) VALUES(?,?,?,?,?) ON CONFLICT(kind,instance_id,record_id) DO UPDATE SET guard=excluded.guard,payload=excluded.payload")
            .bind(record.kind).bind(instance).bind(record.id).bind(guard).bind(record.payload).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Only the scanner can carry completed cache rows over its own revision bump.
/// Async writers still require the exact current revision. Identity, location,
/// bytes and entry-to-file associations must remain unchanged, and opaque
/// payload text is copied exactly rather than round-tripped through typed rows.
pub(crate) async fn rebase_scanned_evidence(
    handle: &Handle,
    instance: &str,
    before: &HashMap<String, Guard>,
    after: &HashMap<String, Guard>,
    before_entries: &HashMap<String, String>,
    after_entries: &HashMap<String, String>,
) {
    let Some(database) = cache(handle).await else {
        return;
    };
    let Ok(live) = current(handle, instance).await else {
        return;
    };
    let Ok(rows) = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT kind,record_id,guard,payload FROM content_evidence WHERE instance_id=? AND kind IN ('check','presentation','fingerprint')"
    ).bind(instance).fetch_all(database.pool()).await else { return };
    for (kind, id, saved, payload) in rows {
        let candidate = (|| {
            let file = if kind == "check" {
                let previous = before_entries.get(&id)?;
                if after_entries.get(&id) != Some(previous) {
                    return None;
                }
                previous
            } else {
                &id
            };
            let previous = before.get(file)?;
            let current = after.get(file)?;
            if previous.revision == current.revision
                || live.get(file) != Some(current)
            {
                return None;
            }
            let mut carried = previous.clone();
            carried.revision = current.revision;
            if &carried != current
                || serde_json::from_str::<Guard>(&saved).ok().as_ref()
                    != Some(previous)
            {
                return None;
            }
            let raw: serde_json::Value = serde_json::from_str(&payload).ok()?;
            if ["schema_version", "schemaVersion", "protocol"]
                .iter()
                .any(|key| raw.get(*key).is_some())
            {
                return None;
            }
            match kind.as_str() {
                "check" => {
                    serde_json::from_value::<
                        super::content_refresh_checks::RefreshCheckRecord,
                    >(raw)
                    .ok()?;
                }
                "presentation" => {
                    serde_json::from_value::<super::content_refresh_presentations::PresentationRecord>(raw).ok()?;
                }
                "fingerprint" => {
                    let record = serde_json::from_value::<
                        super::content_file_fingerprints::FileFingerprintRecord,
                    >(raw)
                    .ok()?;
                    if !record.valid() {
                        return None;
                    }
                }
                _ => return None,
            }
            serde_json::to_string(current).ok()
        })();
        let Some(next) = candidate else { continue };
        // A writer that already installed the new generation wins. Never copy
        // an older payload over its result merely because a scan finished.
        if let Err(error) = sqlx::query("UPDATE content_evidence SET guard=? WHERE kind=? AND instance_id=? AND record_id=? AND guard=? AND payload=?")
            .bind(next).bind(kind).bind(instance).bind(id).bind(saved).bind(payload).execute(database.pool()).await {
            tracing::warn!(%error,"Unchanged scan evidence could not be retained");
        }
    }
}

pub(crate) async fn write<T: Serialize>(
    handle: &Handle,
    kind: &str,
    id: &str,
    expected: &Guard,
    record: &T,
) {
    let valid = current(handle, &expected.instance_id)
        .await
        .ok()
        .and_then(|rows| rows.get(&expected.file_id).cloned());
    if valid.as_ref() != Some(expected) {
        return;
    }
    let Some(database) = cache(handle).await else {
        return;
    };
    let payload = match serde_json::to_string(record) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error,"Content evidence could not be encoded");
            return;
        }
    };
    let guard = match serde_json::to_string(expected) {
        Ok(value) => value,
        Err(_) => return,
    };
    if let Err(error)=sqlx::query("INSERT INTO content_evidence(kind,instance_id,record_id,guard,payload) VALUES(?,?,?,?,?) ON CONFLICT(kind,instance_id,record_id) DO UPDATE SET guard=excluded.guard,payload=excluded.payload")
        .bind(kind).bind(&expected.instance_id).bind(id).bind(guard).bind(payload).execute(database.pool()).await {
        tracing::warn!(%error,"Content evidence write dropped");
    }
}

pub(crate) async fn remove(
    handle: &Handle,
    kind: &str,
    instance: &str,
    id: &str,
) {
    if let Some(database) = cache(handle).await {
        if let Err(error)=sqlx::query("DELETE FROM content_evidence WHERE kind=? AND instance_id=? AND record_id=?")
            .bind(kind).bind(instance).bind(id).execute(database.pool()).await {tracing::warn!(%error,"Content evidence cleanup deferred");}
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Parsed {
    pub metadata: Option<String>,
    pub icon: Option<String>,
}

pub(crate) async fn hydrate_files(
    handle: &Handle,
    instance: &str,
    files: &mut [crate::state::instances::InstanceFile],
) -> crate::Result<()> {
    let guards = current(handle, instance).await?;
    hydrate_with_guards(handle, instance, files, &guards).await;
    Ok(())
}

pub(crate) async fn hydrate_with_guards(
    handle: &Handle,
    instance: &str,
    files: &mut [crate::state::instances::InstanceFile],
    guards: &HashMap<String, Guard>,
) {
    let parsed =
        read_scoped::<Parsed>(handle, "parsed", instance, guards, None).await;
    let artifacts = artifacts(handle, guards.values()).await;
    for file in files {
        file.local_mod_data = None;
        file.icon_path = None;
        let Some(guard) = guards.get(&file.id).filter(|guard| {
            guard.relative_path == file.relative_path
                && guard.sha1.eq_ignore_ascii_case(&file.sha1)
                && u64::try_from(guard.size).ok() == Some(file.size)
        }) else {
            continue;
        };
        let immutable = artifacts.get(&artifact_key(&guard.sha1, guard.size));
        file.local_mod_data = parsed
            .get(&file.id)
            .and_then(|row| row.metadata.clone())
            .or_else(|| immutable.and_then(|row| row.metadata.clone()));
        file.icon_path = parsed
            .get(&file.id)
            .and_then(|row| row.icon.clone())
            .or_else(|| immutable.and_then(|row| row.icon.clone()));
    }
}

fn artifact_key(sha1: &str, size: i64) -> String {
    format!("{}:{size}", sha1.to_ascii_lowercase())
}

async fn artifacts<'a>(
    handle: &Handle,
    guards: impl Iterator<Item = &'a Guard>,
) -> HashMap<String, Parsed> {
    let Some(database) = cache(handle).await else {
        return HashMap::new();
    };
    let ids = guards
        .map(|guard| artifact_key(&guard.sha1, guard.size))
        .collect::<Vec<_>>();
    let mut result = HashMap::new();
    for ids in ids.chunks(200) {
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT record_id,payload FROM content_evidence WHERE kind='artifact' AND instance_id='' AND record_id IN (",
        );
        let mut separated = query.separated(",");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let Ok(rows) = query
            .build_query_as::<(String, String)>()
            .fetch_all(database.pool())
            .await
        else {
            continue;
        };
        for (id, payload) in rows {
            if let Some(parsed) = decode_artifact(&payload) {
                result.insert(id, parsed);
            }
        }
    }
    result
}

fn decode_artifact(payload: &str) -> Option<Parsed> {
    let raw: serde_json::Value = serde_json::from_str(payload).ok()?;
    if ["schema_version", "schemaVersion", "protocol"]
        .iter()
        .any(|key| raw.get(*key).is_some())
    {
        return None;
    }
    serde_json::from_value(raw).ok()
}

pub(crate) async fn parsed_for_entries(
    pool: &SqlitePool,
    instance: &str,
) -> crate::Result<Vec<(String, String)>> {
    let handle = Handle::new(pool.clone());
    let mut connection = pool.acquire().await?;
    let guards = guards(&mut connection, instance).await?;
    let entries = entry_files(&mut connection, instance).await?;
    drop(connection);
    let parsed =
        read_scoped::<Parsed>(&handle, "parsed", instance, &guards, None).await;
    let immutable = artifacts(&handle, guards.values()).await;
    Ok(entries
        .into_iter()
        .filter_map(|(entry, file)| {
            let guard = guards.get(&file)?;
            let metadata = parsed
                .get(&file)
                .and_then(|row| row.metadata.clone())
                .or_else(|| {
                    immutable
                        .get(&artifact_key(&guard.sha1, guard.size))
                        .and_then(|row| row.metadata.clone())
                })?;
            Some((entry, metadata))
        })
        .collect())
}

pub(crate) async fn parsed_for_artifact(
    pool: &SqlitePool,
    sha1: &str,
    size: i64,
) -> crate::Result<Vec<String>> {
    let handle = Handle::new(pool.clone());
    let rows=sqlx::query_as::<_,(String,String)>("SELECT instance_id,id FROM instance_files WHERE sha1=? COLLATE NOCASE AND size=?")
        .bind(sha1).bind(size).fetch_all(pool).await?;
    let mut result = Vec::new();
    if let Some(parsed) = artifact(&handle, sha1, size).await {
        if let Some(metadata) = parsed.metadata {
            result.push(metadata);
        }
    }
    for (instance, id) in rows {
        if let Some(guard) = current(&handle, &instance).await?.get(&id) {
            if let Some(parsed) =
                read::<Parsed>(&handle, "parsed", &instance, &id, guard).await
            {
                if let Some(metadata) = parsed.metadata {
                    result.push(metadata);
                }
            }
        }
    }
    Ok(result)
}

async fn artifact(handle: &Handle, sha1: &str, size: i64) -> Option<Parsed> {
    let database = cache(handle).await?;
    let id = format!("{}:{size}", sha1.to_ascii_lowercase());
    let payload=sqlx::query_scalar::<_,String>("SELECT payload FROM content_evidence WHERE kind='artifact' AND instance_id='' AND record_id=?").bind(id).fetch_optional(database.pool()).await.ok().flatten()?;
    decode_artifact(&payload)
}

/// Parsing describes immutable bytes, so an orphan cache write after a rolled
/// back install is harmless. No ownership or instance selection is stored here.
pub(crate) async fn remember_parsed(
    connection: &mut SqliteConnection,
    sha1: &str,
    size: i64,
    metadata: Option<&str>,
    icon: Option<&str>,
) {
    if metadata.is_none() && icon.is_none() {
        return;
    }
    if sha1.len() != 40
        || !sha1.bytes().all(|byte| byte.is_ascii_hexdigit())
        || size < 0
    {
        return;
    }
    let file = sqlx::query_scalar::<_, String>(
        "SELECT file FROM pragma_database_list WHERE name='main'",
    )
    .fetch_one(&mut *connection)
    .await
    .ok();
    let Some(path) = file.map(PathBuf::from).filter(|path| {
        path.file_name().is_some_and(|name| name == "system.db")
    }) else {
        return;
    };
    let Some(root) = path.parent() else {
        return;
    };
    let Some(database) = cache_at(root.join("derived.db")).await else {
        return;
    };
    let parsed = Parsed {
        metadata: metadata.map(str::to_owned),
        icon: icon.map(str::to_owned),
    };
    if let Err(error) = save_artifact(&database, sha1, size, &parsed).await {
        tracing::warn!(%error,"Parsed content cache write dropped");
    }
}

async fn save_artifact(
    database: &DerivedDb,
    sha1: &str,
    size: i64,
    parsed: &Parsed,
) -> crate::Result<()> {
    let payload = serde_json::to_string(parsed)?;
    sqlx::query("INSERT INTO content_evidence(kind,instance_id,record_id,guard,payload) VALUES('artifact','',?,'',?) ON CONFLICT(kind,instance_id,record_id) DO UPDATE SET payload=excluded.payload")
        .bind(artifact_key(sha1,size)).bind(payload).execute(database.pool()).await?;
    Ok(())
}

#[cfg(test)]
pub(crate) async fn seed_artifact(
    handle: &Handle,
    sha1: &str,
    size: i64,
    metadata: String,
) -> crate::Result<()> {
    let database = cache(handle).await.ok_or_else(|| {
        crate::ErrorKind::FSError("Test evidence unavailable".into())
    })?;
    save_artifact(
        &database,
        sha1,
        size,
        &Parsed {
            metadata: Some(metadata),
            icon: None,
        },
    )
    .await
}
