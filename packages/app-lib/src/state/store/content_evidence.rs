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
    cache_at(
        super::persistence_for_pool(handle.pool())
            .root()
            .join("databases/derived.db"),
    )
    .await
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
            if serde_json::from_str::<Guard>(&guard).ok().as_ref()
                != Some(expected)
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
            Some((id, serde_json::from_value(raw).ok()?))
        })
        .collect()
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
    let Ok(current) = current(handle, instance).await else {
        return;
    };
    let Some(database) = cache(handle).await else {
        return;
    };
    let Ok(mut tx) = database.pool().begin().await else {
        return;
    };
    for record in records {
        if current.get(&record.guard.file_id) != Some(&record.guard) {
            continue;
        }
        let Ok(guard) = serde_json::to_string(&record.guard) else {
            continue;
        };
        if let Err(error)=sqlx::query("INSERT INTO content_evidence(kind,instance_id,record_id,guard,payload) VALUES(?,?,?,?,?) ON CONFLICT(kind,instance_id,record_id) DO UPDATE SET guard=excluded.guard,payload=excluded.payload")
            .bind(record.kind).bind(instance).bind(record.id).bind(guard).bind(record.payload).execute(&mut *tx).await {tracing::warn!(%error,"Evidence batch dropped");return;}
    }
    if let Err(error) = tx.commit().await {
        tracing::warn!(%error,"Evidence batch commit dropped");
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
    let Ok(payload) = serde_json::to_string(&parsed) else {
        return;
    };
    if let Err(error)=sqlx::query("INSERT INTO content_evidence(kind,instance_id,record_id,guard,payload) VALUES('artifact','',?,'',?) ON CONFLICT(kind,instance_id,record_id) DO UPDATE SET payload=excluded.payload")
        .bind(format!("{}:{size}",sha1.to_ascii_lowercase())).bind(payload).execute(database.pool()).await {tracing::warn!(%error,"Parsed content cache write dropped");}
}
