//! Immutable filesystem recovery payloads selected by compact core decisions.
use futures::TryStreamExt;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use sqlx::{
    Column, Connection, Executor, Row, SqliteConnection, TypeInfo, ValueRef,
};
use std::path::{Path, PathBuf};

pub(crate) async fn install(
    connection: &mut SqliteConnection,
) -> Result<(), sqlx::Error> {
    connection.execute("CREATE TABLE IF NOT EXISTS main.operation_publications(kind TEXT NOT NULL, id TEXT NOT NULL, instance_id TEXT, revision INTEGER NOT NULL, object_path TEXT NOT NULL, sha256 TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(kind,id))").await?;
    connection.execute("CREATE INDEX IF NOT EXISTS main.operation_publications_owner ON operation_publications(instance_id,kind)").await?;
    connection.execute("CREATE TABLE IF NOT EXISTS main.operation_imports(kind TEXT PRIMARY KEY NOT NULL)").await?;
    connection.execute("CREATE TABLE IF NOT EXISTS main.operation_opaque(kind TEXT PRIMARY KEY NOT NULL)").await?;
    connection.execute("CREATE TABLE IF NOT EXISTS main.operation_detached_owners(instance_id TEXT PRIMARY KEY NOT NULL)").await?;
    Ok(())
}
pub(crate) async fn active(
    connection: &mut SqliteConnection,
) -> crate::Result<bool> {
    Ok(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM main.sqlite_schema WHERE name='operation_publications')").fetch_one(connection).await?)
}
async fn root(connection: &mut SqliteConnection) -> crate::Result<PathBuf> {
    let file: String = sqlx::query_scalar(
        "SELECT file FROM pragma_database_list WHERE name='main'",
    )
    .fetch_one(connection)
    .await?;
    Path::new(&file)
        .parent()
        .and_then(Path::parent)
        .map(Path::to_owned)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Recovery persistence needs the original data directory".into(),
            )
            .into()
        })
}
fn invalid(message: &str) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

/// A committed publication decision detached from its SQL connection. Payloads
/// have unique immutable paths, so a later replacement or tombstone cannot
/// change what this selection reads. Online publication retains old objects.
#[derive(sqlx::FromRow)]
pub(crate) struct SelectedRecord {
    pub(crate) key: String,
    kind: String,
    instance_id: Option<String>,
    revision: i64,
    object_path: String,
    sha256: String,
    opaque: bool,
}

impl SelectedRecord {
    pub(crate) async fn decode<T: DeserializeOwned>(&self) -> crate::Result<T> {
        if self.opaque {
            return Err(invalid(
                "Legacy operation extension preserved without replay",
            ));
        }
        #[cfg(test)]
        snapshot_pause::before_payload().await;
        let value = crate::state::recovery::record::read(
            Path::new(&self.object_path),
            &self.sha256,
            &self.kind,
            &self.key,
            self.instance_id.as_deref(),
            self.revision,
        )
        .await?;
        validate_value(&self.kind, &value)?;
        Ok(serde_json::from_value(value)?)
    }
}

/// Captures one SQL statement's decisions, including opacity, without doing any
/// filesystem work. The caller releases its connection before decoding them.
pub(crate) async fn select_records(
    connection: &mut SqliteConnection,
    kind: &str,
    key: Option<&str>,
    owner: Option<&str>,
) -> crate::Result<Vec<SelectedRecord>> {
    // Legacy envelopes name their source table inside the payload; they need
    // the existing legacy reader rather than a kind-level opacity decision.
    if kind == "$legacy" {
        return Err(invalid(
            "Legacy envelopes require source-aware interpretation",
        ));
    }
    Ok(sqlx::query_as("SELECT id AS key, kind, instance_id, revision, object_path, sha256, EXISTS(SELECT 1 FROM main.operation_opaque opaque WHERE opaque.kind=publication.kind) AS opaque FROM main.operation_publications publication WHERE kind=? AND deleted=0 AND (? IS NULL OR id=?) AND (? IS NULL OR instance_id=?) ORDER BY id")
        .bind(kind).bind(key).bind(key).bind(owner).bind(owner)
        .fetch_all(connection).await?)
}

/// `None` selects the legacy SQL reader; an empty snapshot means the current
/// file authority selected no records. The connection is gone before return.
pub(crate) async fn snapshot_records(
    pool: &sqlx::SqlitePool,
    kind: &str,
    key: Option<&str>,
    owner: Option<&str>,
) -> crate::Result<Option<Vec<SelectedRecord>>> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(None);
    }
    Ok(Some(
        select_records(&mut connection, kind, key, owner).await?,
    ))
}

/// Freeze the plan and its journals in one main-database statement. Filtering
/// by journal owner also selects the matching plans, rather than all plans.
pub(crate) async fn snapshot_resource_pairs(
    pool: &sqlx::SqlitePool,
    key: Option<&str>,
    owner: Option<&str>,
) -> crate::Result<Option<Vec<SelectedRecord>>> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(None);
    }
    Ok(Some(sqlx::query_as("SELECT id AS key, kind, instance_id, revision, object_path, sha256, EXISTS(SELECT 1 FROM main.operation_opaque opaque WHERE opaque.kind=publication.kind) AS opaque FROM main.operation_publications publication WHERE deleted=0 AND kind IN ('resource_operation_plans','resource_execution_journals') AND (? IS NULL OR id=?) AND (? IS NULL OR (kind='resource_execution_journals' AND instance_id=?) OR (kind='resource_operation_plans' AND id IN (SELECT id FROM main.operation_publications WHERE kind='resource_execution_journals' AND instance_id=? AND deleted=0))) ORDER BY kind,id")
        .bind(key).bind(key).bind(owner).bind(owner).bind(owner)
        .fetch_all(&mut *connection).await?))
}

impl SelectedRecord {
    pub(crate) fn kind(&self) -> &str {
        &self.kind
    }
}

#[cfg(test)]
pub(crate) mod snapshot_pause {
    tokio::task_local! {
        pub(crate) static PAUSE: (std::sync::Arc<tokio::sync::Notify>, std::sync::Arc<tokio::sync::Notify>);
    }

    pub(crate) async fn before_payload() {
        if let Ok(wait) = PAUSE.try_with(|(reached, resume)| {
            reached.notify_one();
            std::sync::Arc::clone(resume)
        }) {
            wait.notified().await;
        }
    }
}
pub(crate) async fn read_raw(
    connection: &mut SqliteConnection,
    kind: &str,
    key: &str,
) -> crate::Result<Option<Value>> {
    let row:Option<(Option<String>,i64,String,String,bool)>=sqlx::query_as("SELECT instance_id,revision,object_path,sha256,deleted FROM main.operation_publications WHERE kind=? AND id=?").bind(kind).bind(key).fetch_optional(&mut *connection).await?;
    let Some((owner, revision, path, hash, deleted)) = row else {
        return Ok(None);
    };
    if deleted {
        return Ok(None);
    }
    let value = crate::state::recovery::record::read(
        Path::new(&path),
        &hash,
        kind,
        key,
        owner.as_deref(),
        revision,
    )
    .await?;
    Ok(Some(value))
}
pub(crate) async fn read<T: DeserializeOwned>(
    connection: &mut SqliteConnection,
    kind: &str,
    key: &str,
) -> crate::Result<Option<T>> {
    let Some(value) = read_raw(connection, kind, key).await? else {
        return Ok(None);
    };
    let source_kind = if kind == "$legacy" {
        value.get("table").and_then(Value::as_str).unwrap_or(kind)
    } else {
        kind
    };
    let opaque: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM main.operation_opaque WHERE kind=?)",
    )
    .bind(source_kind)
    .fetch_one(&mut *connection)
    .await?;
    if opaque {
        return Err(invalid(
            "Legacy operation extension preserved without replay",
        ));
    }
    validate_value(kind, &value)?;
    Ok(Some(serde_json::from_value(value)?))
}
pub(crate) async fn all<T: DeserializeOwned>(
    connection: &mut SqliteConnection,
    kind: &str,
) -> crate::Result<Vec<(String, T)>> {
    let keys:Vec<String>=sqlx::query_scalar("SELECT id FROM main.operation_publications WHERE kind=? AND deleted=0 ORDER BY id").bind(kind).fetch_all(&mut *connection).await?;
    let mut records = Vec::new();
    for key in keys {
        match read(connection, kind, &key).await {
            Ok(Some(record)) => records.push((key, record)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%kind,%key,%error,"Recovery record unavailable; original retained and other records continue")
            }
        }
    }
    Ok(records)
}
pub(crate) async fn write<T: Serialize>(
    connection: &mut SqliteConnection,
    kind: &str,
    key: &str,
    record: Option<&T>,
) -> crate::Result<()> {
    write_inner(connection, kind, key, record, false).await
}
async fn write_imported<T: Serialize>(
    connection: &mut SqliteConnection,
    kind: &str,
    key: &str,
    record: Option<&T>,
) -> crate::Result<()> {
    write_inner(connection, kind, key, record, true).await
}
async fn write_inner<T: Serialize>(
    connection: &mut SqliteConnection,
    kind: &str,
    key: &str,
    record: Option<&T>,
    import: bool,
) -> crate::Result<()> {
    if !import {
        let opaque: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM main.operation_opaque WHERE kind=?)",
        )
        .bind(kind)
        .fetch_one(&mut *connection)
        .await?;
        if opaque {
            return Err(invalid(
                "Legacy extension retained read-only for this operation kind",
            ));
        }
    }
    let previous:Option<(Option<String>,i64,String,String,bool)>=sqlx::query_as("SELECT instance_id,revision,object_path,sha256,deleted FROM main.operation_publications WHERE kind=? AND id=?").bind(kind).bind(key).fetch_optional(&mut *connection).await?;
    let revision = previous
        .as_ref()
        .map(|(_, revision, _, _, _)| *revision)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| invalid("Operation revision overflow"))?;
    let Some(record) = record else {
        if let Some((_, _, _, _, false)) = &previous {
            let _: Value = read(connection, kind, key)
                .await?
                .ok_or_else(|| invalid("Operation record disappeared"))?;
        }
        sqlx::query("UPDATE main.operation_publications SET revision=?,deleted=1 WHERE kind=? AND id=?").bind(revision).bind(kind).bind(key).execute(connection).await?;
        return Ok(());
    };
    let mut value = serde_json::to_value(record)?;
    if let Some((owner, old_revision, path, hash, false)) = &previous {
        let mut old = crate::state::recovery::record::read(
            Path::new(path),
            hash,
            kind,
            key,
            owner.as_deref(),
            *old_revision,
        )
        .await?;
        if let (Some(old), Some(current)) =
            (old.as_object_mut(), value.as_object())
        {
            for (name, value) in current {
                let mut next = value.clone();
                if matches!(name.as_str(), "state" | "payload")
                    && let (Some(previous), Some(incoming)) =
                        (old.get(name).and_then(Value::as_str), value.as_str())
                {
                    let previous: Value = serde_json::from_str(previous)?;
                    let mut incoming: Value = serde_json::from_str(incoming)?;
                    if kind == "install_jobs" && name == "state" {
                        let known =
                            serde_json::to_value(serde_json::from_value::<
                                crate::install::model::InstallJobState,
                            >(
                                previous.clone()
                            )?)?;
                        retain_extensions(&previous, &known, &mut incoming);
                    } else {
                        retain_unknown(&previous, &mut incoming);
                    }
                    next = Value::String(serde_json::to_string(&incoming)?);
                }
                old.insert(name.clone(), next);
            }
            value = Value::Object(old.clone());
        }
    }
    // Newer inner protocols must never be silently rounded down by a typed write.
    if !import {
        if let Some((owner, old_revision, path, hash, false)) = &previous {
            let previous = crate::state::recovery::record::read(
                Path::new(path),
                hash,
                kind,
                key,
                owner.as_deref(),
                *old_revision,
            )
            .await?;
            validate_value(kind, &previous)?;
        }
        validate_value(kind, &value)?;
    }
    if !import {
        for field in ["payload", "state"] {
            if let Some(text) = value.get(field).and_then(Value::as_str) {
                let payload: Value = serde_json::from_str(text)?;
                for name in ["schema_version", "schemaVersion"] {
                    if payload
                        .get(name)
                        .is_some_and(|value| value.as_u64() != Some(1))
                    {
                        return Err(invalid(
                            "Newer operation payload retained read-only",
                        ));
                    }
                }
            }
        }
    }
    let owner = value
        .get("instance_id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            if matches!(
                kind,
                "content_file_operations" | "resource_legacy_rollback_journals"
            ) {
                Some(key.into())
            } else {
                previous
                    .as_ref()
                    .and_then(|(owner, _, _, _, _)| owner.clone())
            }
        });
    let owner = if owner.is_none() && kind == "resource_entry_graphs" {
        sqlx::query_scalar::<_,String>("SELECT instance_id FROM instances.instance_content_entries WHERE id=?").bind(key).fetch_optional(&mut *connection).await?
    } else {
        owner
    };
    let owner = if owner.is_none() && kind == "resource_execution_journals" {
        read::<Value>(connection, "resource_operation_plans", key)
            .await?
            .and_then(|value| value["instance_id"].as_str().map(str::to_owned))
    } else {
        owner
    };
    let root = root(connection).await?;
    let mut boundary = root.clone();
    let mut directory = root
        .join(crate::state::layout::EXTERNAL_OPERATIONS)
        .join(crate::state::instances::manifest::id_key(
            owner.as_deref().unwrap_or("_launcher"),
        ))
        .join("operations");
    let detached = if let Some(owner) = owner.as_deref() {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM main.operation_detached_owners WHERE instance_id=?)")
            .bind(owner).fetch_one(&mut *connection).await?
    } else {
        false
    };
    if let Some(owner) = owner.as_deref().filter(|_| !detached) {
        let row:Option<(String,Option<String>,Option<String>,Option<String>)>=sqlx::query_as("SELECT path,symlink_target,game_dir_override,linked_dot_minecraft FROM instances.instances WHERE id=?").bind(owner).fetch_optional(&mut *connection).await?;
        if let Some((path, None, None, None)) = row {
            let content = super::sync_documents::configured_root(&root)
                .await?
                .join("profiles");
            let profile = content.join(path);
            if let (Ok(base), Ok(actual)) = (
                tokio::fs::canonicalize(&content).await,
                tokio::fs::canonicalize(&profile).await,
            ) {
                if actual.starts_with(&base)
                    && actual != base
                    && !tokio::fs::symlink_metadata(&profile)
                        .await?
                        .file_type()
                        .is_symlink()
                {
                    boundary = actual.clone();
                    directory = actual.join(".axolotl/operations");
                }
            }
        }
    }
    let canonical = owned_directory(&boundary, &directory).await?;
    let path = canonical.join(format!(
        "{}-{}.json",
        crate::state::recovery::record::digest(
            format!("{kind}:{key}").as_bytes()
        ),
        uuid::Uuid::new_v4()
    ));
    let hash = crate::state::recovery::record::write(
        &path,
        kind,
        key,
        owner.as_deref(),
        revision,
        value,
    )
    .await?;
    sqlx::query("INSERT INTO main.operation_publications(kind,id,instance_id,revision,object_path,sha256,deleted) VALUES(?,?,?,?,?,?,0) ON CONFLICT(kind,id) DO UPDATE SET instance_id=excluded.instance_id,revision=excluded.revision,object_path=excluded.object_path,sha256=excluded.sha256,deleted=0").bind(kind).bind(key).bind(owner).bind(revision).bind(path.to_string_lossy().into_owned()).bind(hash).execute(connection).await?;
    Ok(())
}

type RemovalRecord = (String, String, Option<String>, i64, String, String);

/// A routing fence retains operation history even if deleting the profile
/// fails. It never marks a task complete or authorizes file recovery.
pub(crate) async fn preserve_instance_before_removal(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
    removed_path: &Path,
) -> crate::Result<()> {
    preserve_instance_before_removal_with_hook(
        pool,
        instance_id,
        removed_path,
        || async {},
    )
    .await
}

async fn preserve_instance_before_removal_with_hook<F, Fut>(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
    removed_path: &Path,
    before_publish: F,
) -> crate::Result<()>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let (settings_root, records) = {
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        if !active(&mut transaction).await? {
            return Ok(());
        }
        sqlx::query("INSERT OR IGNORE INTO main.operation_detached_owners(instance_id) VALUES(?)")
            .bind(instance_id).execute(&mut *transaction).await?;
        let settings_root = root(&mut transaction).await?;
        let records = removal_records(&mut transaction, instance_id).await?;
        transaction.commit().await?;
        (settings_root, records)
    };
    if records.is_empty() {
        return Ok(());
    }
    let removed_path = match tokio::fs::canonicalize(removed_path).await {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let mut replacements = Vec::new();
    for (kind, key, owner, revision, path, hash) in &records {
        if !Path::new(path).starts_with(&removed_path) {
            continue;
        }
        let directory = settings_root
            .join(crate::state::layout::EXTERNAL_OPERATIONS)
            .join(crate::state::instances::manifest::id_key(instance_id))
            .join("operations")
            .join(uuid::Uuid::new_v4().to_string());
        if tokio::fs::canonicalize(&settings_root)
            .await?
            .starts_with(&removed_path)
        {
            return Err(invalid(
                "Operation preservation destination is inside the removal tree",
            ));
        }
        let directory = owned_directory(&settings_root, &directory).await?;
        if directory.starts_with(&removed_path) {
            return Err(invalid(
                "Operation preservation destination is inside the removal tree",
            ));
        }
        let destination = directory.join(
            Path::new(path)
                .file_name()
                .ok_or_else(|| invalid("Operation filename is missing"))?,
        );
        crate::state::recovery::record::copy_exact(
            Path::new(path),
            &destination,
            hash,
            kind,
            key,
            owner.as_deref(),
            *revision,
        )
        .await?;
        replacements.push((kind, key, destination));
    }
    before_publish().await;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if removal_records(&mut transaction, instance_id).await? != records {
        return Err(invalid(
            "Operation changed while preparing instance removal; retry removal",
        ));
    }
    for (kind, key, destination) in replacements {
        sqlx::query("UPDATE main.operation_publications SET object_path=? WHERE kind=? AND id=?")
            .bind(destination.to_string_lossy().into_owned()).bind(kind).bind(key)
            .execute(&mut *transaction).await?;
    }
    transaction.commit().await?;
    Ok(())
}

async fn removal_records(
    connection: &mut SqliteConnection,
    instance_id: &str,
) -> crate::Result<Vec<RemovalRecord>> {
    Ok(sqlx::query_as("SELECT kind,id,instance_id,revision,object_path,sha256 FROM main.operation_publications WHERE instance_id=? AND deleted=0 ORDER BY kind,id")
        .bind(instance_id).fetch_all(connection).await?)
}

pub(crate) async fn ensure_instance_supported(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
) -> crate::Result<()> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let keys:Vec<(String,String)>=sqlx::query_as("SELECT kind,id FROM main.operation_publications WHERE instance_id=? AND deleted=0").bind(instance_id).fetch_all(&mut *connection).await?;
    for (kind, key) in keys {
        ensure_known_kind(&kind)?;
        let value: Value = read(&mut connection, &kind, &key)
            .await?
            .ok_or_else(|| invalid("Missing selected operation"))?;
        for field in ["payload", "state"] {
            if let Some(text) = value.get(field).and_then(Value::as_str) {
                let _: Value = super::compatibility::decode_payload(text)?;
            }
        }
    }
    Ok(())
}
pub(crate) async fn ensure_cleanup_supported(
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let opaque: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM main.operation_opaque)",
    )
    .fetch_one(&mut *connection)
    .await?;
    if opaque {
        return Err(invalid(
            "Opaque legacy operation attachments prevent destructive cleanup",
        ));
    }
    let keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT kind,id FROM main.operation_publications WHERE deleted=0",
    )
    .fetch_all(&mut *connection)
    .await?;
    for (kind, key) in keys {
        ensure_known_kind(&kind)?;
        let value: Value =
            read(&mut connection, &kind, &key).await?.ok_or_else(|| {
                invalid("Missing operation authority prevents cleanup")
            })?;
        for field in ["payload", "state"] {
            if let Some(text) = value.get(field).and_then(Value::as_str) {
                let _: Value = super::compatibility::decode_payload(text)?;
            }
        }
    }
    Ok(())
}

/// Copy every selected immutable generation, including retained damaged files.
/// Read decisions from the frozen core snapshot, never the live database whose
/// selected operation may change while these immutable objects are copied.
/// A backup keeps the SQL snapshot's object-path mapping in a portable manifest.
pub(crate) async fn backup(
    snapshot: &Path,
    staging: &Path,
) -> crate::Result<()> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(snapshot)
                .read_only(true),
        )
        .await?;
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let records:Vec<(String,String,Option<String>,i64,String,String,bool)>=sqlx::query_as("SELECT kind,id,instance_id,revision,object_path,sha256,deleted FROM main.operation_publications ORDER BY kind,id").fetch_all(&mut *connection).await?;
    let destination = staging.join("operation-records");
    for (_, _, _, _, path, _, deleted) in &records {
        if *deleted {
            continue;
        }
        let path = Path::new(path);
        let parent = path
            .parent()
            .ok_or_else(|| invalid("Operation envelope parent unavailable"))?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| invalid("Operation envelope filename invalid"))?;
        let folder = destination.join(crate::state::recovery::record::digest(
            path.to_string_lossy().as_bytes(),
        ));
        // The live instance may be removed after the core snapshot was frozen.
        // Missing selected files invalidate this backup; retaining only their
        // SQL references would falsely advertise a complete recovery source.
        let mut entries = tokio::fs::read_dir(parent).await?;
        let mut envelope = None;
        let mut previous_envelope = false;
        let previous_name = format!(
            "{}.previous",
            path.file_name()
                .expect("envelope filename")
                .to_string_lossy()
        );
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with(stem)
                || name.to_string_lossy().ends_with(".lock")
                || !entry.file_type().await?.is_file()
            {
                continue;
            }
            if entry.path() == path {
                envelope = Some((entry.path(), folder.join(name)));
                continue;
            }
            if name.to_string_lossy() == previous_name {
                previous_envelope = true;
            }
            crate::state::persistence::document::write_blob(
                &folder.join(name),
                crate::state::recovery::record::bounded(
                    &entry.path(),
                    2 * 1024 * 1024,
                )
                .await?,
            )
            .await?;
        }
        if envelope.is_none() && !previous_envelope {
            return Err(invalid(
                "Selected operation envelope disappeared during backup",
            ));
        }
        if let Some((source, target)) = envelope {
            crate::state::persistence::document::write_blob(
                &target,
                crate::state::recovery::record::bounded(
                    &source,
                    2 * 1024 * 1024,
                )
                .await?,
            )
            .await?;
        }
    }
    crate::state::persistence::document::write_json(
        &destination.join("references.json"),
        serde_json::to_vec(&records)?,
    )
    .await?;
    Ok(())
}

const IMPORT_KINDS: [&str; 10] = [
    "install_jobs",
    "resource_operation_plans",
    "resource_execution_journals",
    "resource_history_cleanup_plans",
    "resource_entry_graphs",
    "content_update_jobs",
    "content_file_operations",
    "pending_backup_repository_cleanups",
    "resource_legacy_rollback_journals",
    "$legacy-extra",
];

async fn completed_imports(
    connection: &mut SqliteConnection,
) -> crate::Result<std::collections::HashSet<String>> {
    let kinds: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM main.operation_imports")
            .fetch_all(connection)
            .await?;
    Ok(kinds.into_iter().collect())
}

pub(crate) async fn legacy_import_pending(
    pool: &sqlx::SqlitePool,
) -> crate::Result<bool> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(false);
    }
    let completed = completed_imports(&mut connection).await?;
    Ok(IMPORT_KINDS.iter().any(|kind| !completed.contains(*kind)))
}

struct ImportProgress<'a> {
    display: Option<&'a mut crate::event::migration::MigrationProgress>,
    completed: u64,
    total: u64,
}
impl ImportProgress<'_> {
    fn advance(&mut self, count: u64) {
        self.completed = self.completed.saturating_add(count).min(self.total);
        if let Some(display) = &mut self.display {
            display.recovery(self.completed, self.total);
        }
    }
}

pub(crate) async fn import_legacy(
    pool: &sqlx::SqlitePool,
    root: &Path,
) -> crate::Result<()> {
    import_legacy_with_progress(pool, root, None).await
}

pub(crate) async fn import_legacy_with_progress(
    pool: &sqlx::SqlitePool,
    root: &Path,
    progress: Option<&mut crate::event::migration::MigrationProgress>,
) -> crate::Result<()> {
    use super::Collection;
    let mut destination = pool.acquire().await?;
    if !active(&mut destination).await? {
        return Ok(());
    }
    let completed = completed_imports(&mut destination).await?;
    if IMPORT_KINDS.iter().all(|kind| completed.contains(*kind)) {
        return Ok(());
    }
    let source = super::operations_db::legacy(root).await?;
    if source.is_none()
        && ["app.db", "release/app.db", "beta/app.db"]
            .iter()
            .any(|path| root.join(path).is_file())
    {
        return Err(invalid(
            "Legacy operation source is unavailable; empty handover was refused",
        ));
    }
    let mut extra_tables = vec![
        "install_job_items".to_owned(),
        "scheduler_tags".to_owned(),
        "scheduler_restart_budget".to_owned(),
        "resource_legacy_rollback_journals".to_owned(),
    ];
    let mut counts = std::collections::HashMap::<String, u64>::new();
    if let Some(source) = &source {
        let frozen: Value =
            serde_json::from_str(include_str!("database_baseline.json"))?;
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_schema WHERE type='table'",
        )
        .fetch_all(source)
        .await?;
        for table in tables {
            if !table.starts_with('_')
                && !table.starts_with("sqlite_")
                && !frozen["tables"].as_array().is_some_and(|known| {
                    known.iter().any(|known| known["name"] == table)
                })
                && !extra_tables.contains(&table)
            {
                extra_tables.push(table.clone());
            }
            if IMPORT_KINDS.contains(&table.as_str())
                || extra_tables.contains(&table)
            {
                let count: i64 =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT COUNT(*) FROM \"{}\"",
                        table.replace('"', "\"\"")
                    )))
                    .fetch_one(source)
                    .await?;
                counts.insert(table, count.max(0) as u64);
            }
        }
    }
    // Include a unit for each durable receipt. Processing every row alone is
    // insufficient to report a completed, crash-safe takeover.
    let mut total = IMPORT_KINDS
        .iter()
        .filter(|kind| !completed.contains(**kind))
        .count() as u64;
    if !completed.contains("$legacy-extra") {
        total += extra_tables
            .iter()
            .map(|table| counts.get(table).copied().unwrap_or(0))
            .sum::<u64>();
    }
    for kind in IMPORT_KINDS
        .iter()
        .filter(|kind| **kind != "$legacy-extra" && !completed.contains(**kind))
    {
        total += counts.get(*kind).copied().unwrap_or(0)
            * if *kind == "resource_legacy_rollback_journals" {
                1
            } else {
                2
            };
    }
    let mut progress = ImportProgress {
        display: progress,
        completed: 0,
        total,
    };
    progress.advance(0);
    macro_rules! import {
        ($kind:literal, $collection:ty) => {{
            if !completed.contains($kind) {
                let raw_count = if let Some(source) = &source {
                    capture_legacy(&mut destination, source, $kind, &mut progress).await?
                } else { 0 };
                let rows = if let Some(source) = &source {
                    let mut connection = source.acquire().await?;
                    match <$collection as Collection>::read_all(&mut connection).await {
                        Ok(rows) => rows,
                        Err(error) => {
                            tracing::warn!(kind=$kind,%error,"Legacy operation rows retained as opaque attachments");
                            sqlx::query("INSERT OR IGNORE INTO main.operation_opaque(kind) VALUES(?)").bind($kind).execute(&mut *destination).await?;
                            Vec::new()
                        }
                    }
                } else { Vec::new() };
                if rows.len() as u64 != raw_count {
                    sqlx::query("INSERT OR IGNORE INTO main.operation_opaque(kind) VALUES(?)").bind($kind).execute(&mut *destination).await?;
                }
                let interpreted = rows.len() as u64;
                let mut transaction = destination.begin_with("BEGIN IMMEDIATE").await?;
                for (key, record) in rows {
                    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.operation_publications WHERE kind=? AND id=?)").bind($kind).bind(&key).fetch_one(&mut *transaction).await?;
                    if !exists { write_imported(&mut transaction, $kind, &key, Some(&record)).await?; }
                    progress.advance(1);
                }
                // Opaque rows have been preserved and examined, not discarded.
                progress.advance(raw_count.saturating_sub(interpreted));
                sqlx::query("INSERT OR IGNORE INTO main.operation_imports(kind) VALUES(?)").bind($kind).execute(&mut *transaction).await?;
                transaction.commit().await?;
                progress.advance(1);
            }
        }};
    }
    if !completed.contains("$legacy-extra") {
        if let Some(source) = &source {
            for table in &extra_tables {
                capture_legacy(&mut destination, source, table, &mut progress)
                    .await?;
            }
        }
        sqlx::query("INSERT OR IGNORE INTO main.operation_imports(kind) VALUES('$legacy-extra')").execute(&mut *destination).await?;
        progress.advance(1);
    }
    import!("install_jobs", super::install_jobs::InstallJobs);
    import!(
        "resource_operation_plans",
        super::resource_operation_plans::ResourceOperationPlans
    );
    import!(
        "resource_execution_journals",
        super::resource_execution_journals::ResourceExecutionJournals
    );
    import!(
        "resource_history_cleanup_plans",
        super::resource_history_cleanup_plans::ResourceHistoryCleanupPlans
    );
    import!(
        "resource_entry_graphs",
        super::resource_entry_graphs::ResourceEntryGraphs
    );
    import!(
        "content_update_jobs",
        super::content_update_jobs::ContentUpdateJobs
    );
    import!(
        "content_file_operations",
        super::content_file_operations::ContentFileOperations
    );
    import!("pending_backup_repository_cleanups", super::pending_backup_repository_cleanups::PendingBackupRepositoryCleanups);
    if !completed.contains("resource_legacy_rollback_journals") {
        let mut transaction = destination.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(source) = &source {
            let rows: Vec<(String, String)> = sqlx::query_as("SELECT instance_id,payload FROM resource_legacy_rollback_journals").fetch_all(source).await?;
            for (key, payload) in rows {
                let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.operation_publications WHERE kind='resource_legacy_rollback_journals' AND id=?)").bind(&key).fetch_one(&mut *transaction).await?;
                if !exists {
                    write_imported(
                        &mut transaction,
                        "resource_legacy_rollback_journals",
                        &key,
                        Some(&serde_json::json!({"payload":payload})),
                    )
                    .await?;
                }
                progress.advance(1);
            }
        }
        sqlx::query("INSERT OR IGNORE INTO main.operation_imports(kind) VALUES('resource_legacy_rollback_journals')").execute(&mut *transaction).await?;
        transaction.commit().await?;
        progress.advance(1);
    }
    Ok(())
}

pub(crate) async fn interpret_instance(
    pool: &sqlx::SqlitePool,
    instance_id: &str,
) -> crate::Result<()> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let keys:Vec<(String,String)>=sqlx::query_as("SELECT kind,id FROM main.operation_publications WHERE instance_id=? AND deleted=0").bind(instance_id).fetch_all(&mut *connection).await?;
    for (kind, key) in keys {
        ensure_known_kind(&kind)?;
        let value: Value = read(&mut connection, &kind, &key)
            .await?
            .ok_or_else(|| invalid("Selected recovery record absent"))?;
        for field in ["payload", "state"] {
            if let Some(text) = value.get(field).and_then(Value::as_str) {
                let _: Value = super::compatibility::decode_payload(text)?;
            }
        }
    }
    Ok(())
}

fn retain_unknown(previous: &Value, incoming: &mut Value) {
    if let (Some(previous), Some(incoming)) =
        (previous.as_object(), incoming.as_object_mut())
    {
        for (key, value) in previous {
            match incoming.get_mut(key) {
                Some(current) => retain_unknown(value, current),
                None => {
                    incoming.insert(key.clone(), value.clone());
                }
            }
        }
    }
}

/// Missing known optional fields represent deletion. Only fields omitted by
/// the old record's typed representation are extensions to carry forward.
fn retain_extensions(previous: &Value, known: &Value, incoming: &mut Value) {
    if let (Some(previous), Some(known), Some(incoming)) = (
        previous.as_object(),
        known.as_object(),
        incoming.as_object_mut(),
    ) {
        for (key, value) in previous {
            match (known.get(key), incoming.get_mut(key)) {
                (None, None) => {
                    incoming.insert(key.clone(), value.clone());
                }
                (Some(known), Some(current)) => {
                    retain_extensions(value, known, current);
                }
                _ => {}
            }
        }
    }
}
pub(crate) async fn strict_all<T: DeserializeOwned>(
    connection: &mut SqliteConnection,
    kind: &str,
) -> crate::Result<Vec<(String, T)>> {
    let keys: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM main.operation_publications WHERE kind=? AND deleted=0",
    )
    .bind(kind)
    .fetch_all(&mut *connection)
    .await?;
    let mut rows = Vec::new();
    for key in keys {
        let record = read(connection, kind, &key)
            .await?
            .ok_or_else(|| invalid("Selected recovery absent"))?;
        rows.push((key, record));
    }
    Ok(rows)
}

pub(crate) async fn interpret_all(
    pool: &sqlx::SqlitePool,
) -> crate::Result<()> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let keys:Vec<(String,String,Option<String>)>=sqlx::query_as("SELECT kind,id,instance_id FROM main.operation_publications WHERE deleted=0").fetch_all(&mut *connection).await?;
    for (kind, key, owner) in keys {
        let interpreted = async {
            let value: Value = read(&mut connection, &kind, &key)
                .await?
                .ok_or_else(|| invalid("Recovery record disappeared"))?;
            for field in ["payload", "state"] {
                if let Some(text) = value.get(field).and_then(Value::as_str) {
                    let _: Value = super::compatibility::decode_payload(text)?;
                }
            }
            Ok::<_, crate::Error>(())
        }
        .await;
        if let Err(error) = interpreted {
            tracing::warn!(%kind,%key,?owner,%error,"Record preserved; interpretation failed locally");
        }
    }
    Ok(())
}

/// Rebind only byte-identical immutable records after the content tree moved.
pub(crate) async fn move_authority(
    pool: &sqlx::SqlitePool,
    settings_root: &Path,
    old_content: &Path,
    new_content: &Path,
) -> crate::Result<()> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let records:Vec<(String,String,Option<String>,i64,String,String)>=sqlx::query_as("SELECT kind,id,instance_id,revision,object_path,sha256 FROM main.operation_publications WHERE deleted=0").fetch_all(&mut *transaction).await?;
    for (kind, key, owner, revision, path, hash) in records {
        if Path::new(&path).starts_with(
            settings_root.join(crate::state::layout::EXTERNAL_OPERATIONS),
        ) {
            continue;
        }
        let Ok(profile_relative) =
            Path::new(&path).strip_prefix(old_content.join("profiles"))
        else {
            continue;
        };
        if !profile_relative
            .components()
            .any(|component| component.as_os_str() == "operations")
        {
            continue;
        }
        let relative = Path::new("profiles").join(profile_relative);
        let next = new_content.join(relative);
        crate::state::recovery::record::read(
            &next,
            &hash,
            &kind,
            &key,
            owner.as_deref(),
            revision,
        )
        .await?;
        sqlx::query("UPDATE main.operation_publications SET object_path=? WHERE kind=? AND id=? AND revision=? AND sha256=?").bind(next.to_string_lossy().into_owned()).bind(kind).bind(key).bind(revision).bind(hash).execute(&mut *transaction).await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn restore_references(
    connection: &mut SqliteConnection,
    root: &Path,
    content_root: &Path,
    bundle: &Path,
) -> crate::Result<()> {
    let source = bundle.join("operation-records");
    let references = source.join("references.json");
    if !tokio::fs::try_exists(&references).await? {
        return Err(invalid("Snapshot lacks operation reference inventory"));
    }
    let mut rows: Vec<(
        String,
        String,
        Option<String>,
        i64,
        String,
        String,
        bool,
    )> = serde_json::from_slice(
        &crate::state::recovery::record::bounded(&references, 16 * 1024 * 1024)
            .await?,
    )?;
    let mut selected:Vec<(String,String,Option<String>,i64,String,String,bool)>=sqlx::query_as("SELECT kind,id,instance_id,revision,object_path,sha256,deleted FROM main.operation_publications").fetch_all(&mut *connection).await?;
    rows.sort();
    selected.sort();
    if rows != selected
        || rows
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1)
    {
        return Err(invalid(
            "Operation backup inventory does not match its core snapshot",
        ));
    }
    for (kind, key, owner, revision, path, hash, deleted) in &rows {
        if *deleted {
            continue;
        }
        ensure_known_kind(kind)?;
        let filename = Path::new(path)
            .file_name()
            .ok_or_else(|| invalid("Invalid operation filename"))?;
        let value = crate::state::recovery::record::read(
            &source
                .join(crate::state::recovery::record::digest(path.as_bytes()))
                .join(filename),
            hash,
            kind,
            key,
            owner.as_deref(),
            *revision,
        )
        .await?;
        if has_file_obligation(kind, &value)
            && !Path::new(path).starts_with(
                root.join(crate::state::layout::EXTERNAL_OPERATIONS),
            )
            && !Path::new(path).starts_with(content_root.join("profiles"))
        {
            return Err(invalid(
                "Snapshot contains path-sensitive pending recovery; restore it to its original layout after recovery completes",
            ));
        }
    }
    for (kind, key, owner, revision, path, hash, deleted) in rows {
        if deleted {
            continue;
        }
        let saved = source
            .join(crate::state::recovery::record::digest(path.as_bytes()));
        let filename = Path::new(&path)
            .file_name()
            .ok_or_else(|| invalid("Invalid backed up operation filename"))?;
        crate::state::recovery::record::read(
            &saved.join(filename),
            &hash,
            &kind,
            &key,
            owner.as_deref(),
            revision,
        )
        .await?;
        let mut boundary = root.to_owned();
        let mut destination = root
            .join(crate::state::layout::EXTERNAL_OPERATIONS)
            .join(crate::state::instances::manifest::id_key(
                owner.as_deref().unwrap_or("_launcher"),
            ))
            .join("operations");
        if let Some(owner) = owner.as_deref() {
            let row:Option<(String,Option<String>,Option<String>,Option<String>)>=sqlx::query_as("SELECT path,symlink_target,game_dir_override,linked_dot_minecraft FROM instances.instances WHERE id=?").bind(owner).fetch_optional(&mut *connection).await?;
            if let Some((path, None, None, None)) = row {
                let profiles = content_root.join("profiles");
                let profile =
                    crate::util::io::safe_relative_join(&profiles, &path)?;
                if let (Ok(base), Ok(actual)) = (
                    tokio::fs::canonicalize(&profiles).await,
                    tokio::fs::canonicalize(&profile).await,
                ) {
                    if actual.starts_with(&base)
                        && actual != base
                        && !tokio::fs::symlink_metadata(&profile)
                            .await?
                            .file_type()
                            .is_symlink()
                    {
                        boundary = actual.clone();
                        destination = actual.join(".axolotl/operations");
                    }
                }
            }
        }
        let destination =
            destination.join(format!("restored-{}", uuid::Uuid::new_v4()));
        let destination = owned_directory(&boundary, &destination).await?;
        let mut entries = tokio::fs::read_dir(&saved).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                return Err(invalid("Unexpected object in operation backup"));
            }
            crate::state::persistence::document::write_blob(
                &destination.join(entry.file_name()),
                crate::state::recovery::record::bounded(
                    &entry.path(),
                    2 * 1024 * 1024,
                )
                .await?,
            )
            .await?;
        }
        let restored = destination.join(filename);
        crate::state::recovery::record::read(
            &restored,
            &hash,
            &kind,
            &key,
            owner.as_deref(),
            revision,
        )
        .await?;
        let updated=sqlx::query("UPDATE main.operation_publications SET object_path=? WHERE kind=? AND id=? AND revision=? AND sha256=?").bind(restored.to_string_lossy().into_owned()).bind(kind).bind(key).bind(revision).bind(hash).execute(&mut *connection).await?;
        if updated.rows_affected() != 1 {
            return Err(invalid(
                "Restored operation reference changed during publication",
            ));
        }
    }
    Ok(())
}

fn validate_value(kind: &str, value: &Value) -> crate::Result<()> {
    for field in ["payload", "state"] {
        if let Some(text) = value.get(field).and_then(Value::as_str) {
            let payload: Value = super::compatibility::decode_payload(text)?;
            if payload
                .pointer("/plan/schemaVersion")
                .is_some_and(|version| version.as_u64() != Some(1))
            {
                return Err(invalid("Newer operation plan retained read-only"));
            }
        }
    }
    if kind == "resource_entry_graphs" {
        for field in ["nodes", "edges"] {
            let text = value[field]
                .as_str()
                .ok_or_else(|| invalid("Dependency graph is unreadable"))?;
            let _: Value = serde_json::from_str(text)?;
        }
    }
    if kind == "install_jobs" {
        let kind = value["kind"]
            .as_str()
            .ok_or_else(|| invalid("Install kind unavailable"))?;
        let status = value["status"]
            .as_str()
            .ok_or_else(|| invalid("Install status unavailable"))?;
        if crate::install::model::InstallJobKind::from_stored_str(kind).as_str()
            != kind
            || crate::install::model::InstallJobStatus::from_stored_str(status)
                .as_str()
                != status
        {
            return Err(invalid("Future install operation retained read-only"));
        }
    }
    Ok(())
}

/// Preserve every original cell before typed import; future columns stay opaque.
async fn capture_legacy(
    destination: &mut SqliteConnection,
    source: &sqlx::SqlitePool,
    table: &str,
    progress: &mut ImportProgress<'_>,
) -> crate::Result<u64> {
    let baseline: Value =
        serde_json::from_str(include_str!("database_baseline.json"))?;
    let expected = baseline["tables"]
        .as_array()
        .and_then(|tables| tables.iter().find(|value| value["name"] == table));
    let mut connection = source.acquire().await?;
    let columns: Vec<String> =
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT name FROM pragma_table_info('{}') ORDER BY cid",
            table.replace('\'', "''")
        )))
        .fetch_all(&mut *connection)
        .await?;
    if columns.is_empty() {
        return Ok(0);
    }
    let expected_columns =
        expected.and_then(|expected| expected["columns"].as_array());
    if expected_columns.is_none_or(|expected_columns| {
        columns.iter().any(|column| {
            !expected_columns
                .iter()
                .any(|expected| expected.as_str() == Some(column))
        })
    }) {
        sqlx::query(
            "INSERT OR IGNORE INTO main.operation_opaque(kind) VALUES(?)",
        )
        .bind(table)
        .execute(&mut *destination)
        .await?;
    }
    let quoted_table = format!("\"{}\"", table.replace('"', "\"\""));
    let size_expression = columns
        .iter()
        .map(|column| {
            format!("COALESCE(length(\"{}\"),0)", column.replace('"', "\"\""))
        })
        .collect::<Vec<_>>()
        .join("+");
    let oversized:bool=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT EXISTS(SELECT 1 FROM {quoted_table} WHERE ({size_expression})>16777216)"))).fetch_one(&mut *connection).await?;
    if oversized {
        return Err(invalid(
            "Legacy operation row exceeds bounded import; original database retained",
        ));
    }
    let mut rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT * FROM {quoted_table}"
    )))
    .fetch(&mut *connection);
    let mut index = 0u64;
    while let Some(row) = rows.try_next().await? {
        let mut cells = serde_json::Map::new();
        for (column_index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(column_index)?;
            let value = if raw.is_null() {
                Value::Null
            } else {
                match raw.type_info().name() {
                    "INTEGER" => {
                        Value::from(row.try_get::<i64, _>(column_index)?)
                    }
                    "REAL" => Value::from(row.try_get::<f64, _>(column_index)?),
                    "BLOB" => {
                        serde_json::json!({"sqlite_blob":row.try_get::<Vec<u8>,_>(column_index)?})
                    }
                    _ => Value::from(row.try_get::<String, _>(column_index)?),
                }
            };
            cells.insert(column.name().into(), value);
        }
        let key = format!("{table}:{index}");
        index += 1;
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.operation_publications WHERE kind='$legacy' AND id=?)").bind(&key).fetch_one(&mut *destination).await?;
        if !exists {
            let owner = cells
                .get("instance_id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            write_imported(destination,"$legacy",&key,Some(&serde_json::json!({"table":table,"instance_id":owner,"cells":cells}))).await?;
        }
        progress.advance(1);
    }
    Ok(index)
}

/// Resolve existing ancestors before creating each child, rejecting links before
/// they could direct even a directory creation outside the owned boundary.
async fn owned_directory(
    boundary: &Path,
    requested: &Path,
) -> crate::Result<PathBuf> {
    use std::path::Component;
    let relative = requested
        .strip_prefix(boundary)
        .map_err(|_| invalid("Operation directory is outside its owner"))?;
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid("Operation path is not a contained relative path"));
    }
    let canonical = tokio::fs::canonicalize(boundary).await?;
    let mut current = canonical.clone();
    for component in relative.components() {
        current.push(component.as_os_str());
        match tokio::fs::symlink_metadata(&current).await {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(invalid(
                        "Operation directory has a link or non-directory ancestor",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir(&current).await?;
            }
            Err(error) => return Err(error.into()),
        }
        let actual = tokio::fs::canonicalize(&current).await?;
        if !actual.starts_with(&canonical) {
            return Err(invalid("Operation directory escaped its owner"));
        }
        current = actual;
    }
    Ok(current)
}

pub(crate) async fn strict_owner<T: DeserializeOwned>(
    connection: &mut SqliteConnection,
    kind: &str,
    owner: &str,
) -> crate::Result<Vec<(String, T)>> {
    let keys:Vec<String>=sqlx::query_scalar("SELECT id FROM main.operation_publications WHERE kind=? AND instance_id=? AND deleted=0").bind(kind).bind(owner).fetch_all(&mut *connection).await?;
    let mut records = Vec::new();
    for key in keys {
        records.push((
            key.clone(),
            read(connection, kind, &key)
                .await?
                .ok_or_else(|| invalid("Selected operation absent"))?,
        ));
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn removal_fixture() -> (tempfile::TempDir, sqlx::SqlitePool, PathBuf)
    {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("profiles/first");
        tokio::fs::create_dir_all(&profile).await.unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        sqlx::query("INSERT INTO instances.instances(id,path,name,install_stage,launcher_feature_version,created,modified) VALUES ('first','first','First','installed','none',1,1),('other','other','Other','installed','none',1,1)")
            .execute(&pool).await.unwrap();
        pool.close().await;
        drop(pool);
        let instance_database = root.path().join("data/instances.db");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .after_connect(move |connection, _| {
                let instance_database = instance_database.clone();
                Box::pin(async move {
                    sqlx::query("ATTACH DATABASE ? AS instances")
                        .bind(instance_database.to_string_lossy().into_owned())
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.path().join("data/system.db")),
            )
            .await
            .unwrap();
        (root, pool, profile)
    }

    async fn removal_record(
        pool: &sqlx::SqlitePool,
        key: &str,
        owner: &str,
        dismissed: bool,
    ) -> RemovalRecord {
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        write_imported(&mut transaction, "example", key, Some(&json!({"instance_id":owner,"dismissed":dismissed,"future":{"retain":true}}))).await.unwrap();
        transaction.commit().await.unwrap();
        removal_records(&mut *pool.acquire().await.unwrap(), owner)
            .await
            .unwrap()
            .into_iter()
            .find(|record| record.1 == key)
            .unwrap()
    }

    #[tokio::test]
    async fn removal_preserves_exact_generations_and_routes_later_writes_outside_profile()
     {
        let (root, pool, profile) = removal_fixture().await;
        let active = removal_record(&pool, "active", "first", false).await;
        let dismissed = removal_record(&pool, "dismissed", "first", true).await;
        let foreign = removal_record(&pool, "foreign", "other", false).await;
        tokio::fs::write(&dismissed.4, b"broken original")
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO main.operation_opaque(kind) VALUES('example')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(
            Path::new(&active.4)
                .starts_with(tokio::fs::canonicalize(&profile).await.unwrap())
        );

        preserve_instance_before_removal(&pool, "first", &profile)
            .await
            .unwrap();
        let selected =
            removal_records(&mut *pool.acquire().await.unwrap(), "first")
                .await
                .unwrap();
        for before in [&active, &dismissed] {
            let after =
                selected.iter().find(|record| record.1 == before.1).unwrap();
            assert_eq!(
                (&after.0, &after.1, &after.2, after.3, &after.5),
                (&before.0, &before.1, &before.2, before.3, &before.5)
            );
            assert!(
                Path::new(&after.4).starts_with(
                    tokio::fs::canonicalize(
                        root.path()
                            .join(crate::state::layout::EXTERNAL_OPERATIONS)
                    )
                    .await
                    .unwrap()
                )
            );
            assert_eq!(
                crate::state::recovery::record::read(
                    Path::new(&after.4),
                    &after.5,
                    &after.0,
                    &after.1,
                    after.2.as_deref(),
                    after.3
                )
                .await
                .unwrap()["future"]["retain"],
                true
            );
        }
        assert_eq!(
            removal_records(&mut *pool.acquire().await.unwrap(), "other")
                .await
                .unwrap(),
            vec![foreign]
        );
        assert_eq!(
            tokio::fs::read(&dismissed.4).await.unwrap(),
            b"broken original"
        );
        // A retry after a failed filesystem deletion does not repoint a record
        // into the still-existing profile or rewrite an already selected copy.
        preserve_instance_before_removal(&pool, "first", &profile)
            .await
            .unwrap();
        assert_eq!(
            removal_records(&mut *pool.acquire().await.unwrap(), "first")
                .await
                .unwrap(),
            selected
        );
        sqlx::query("DELETE FROM main.operation_opaque WHERE kind='example'")
            .execute(&pool)
            .await
            .unwrap();
        let later = removal_record(&pool, "later", "first", false).await;
        assert!(
            !Path::new(&later.4)
                .starts_with(tokio::fs::canonicalize(&profile).await.unwrap())
        );
        tokio::fs::remove_dir_all(&profile).await.unwrap();
        for row in selected.into_iter().chain([later]) {
            assert!(
                crate::state::recovery::record::read(
                    Path::new(&row.4),
                    &row.5,
                    &row.0,
                    &row.1,
                    row.2.as_deref(),
                    row.3
                )
                .await
                .is_ok()
            );
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn removal_rejects_a_changed_generation_without_overwriting_its_new_decision()
     {
        let (_root, pool, profile) = removal_fixture().await;
        removal_record(&pool, "active", "first", false).await;
        let result = preserve_instance_before_removal_with_hook(&pool, "first", &profile, || async {
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
                write_imported(&mut tx, "example", "active", Some(&json!({"instance_id":"first","future":"new generation"}))).await.unwrap();
                tx.commit().await.unwrap();
            }).await.expect("record copying must release the sole connection and core writer");
        }).await;
        assert!(result.is_err());
        assert!(profile.is_dir());
        let row = removal_records(&mut *pool.acquire().await.unwrap(), "first")
            .await
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(row.3, 2);
        assert_eq!(
            crate::state::recovery::record::read(
                Path::new(&row.4),
                &row.5,
                &row.0,
                &row.1,
                row.2.as_deref(),
                row.3
            )
            .await
            .unwrap()["future"],
            "new generation"
        );
        preserve_instance_before_removal(&pool, "first", &profile)
            .await
            .unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn removal_refuses_unrecoverable_or_future_envelopes_without_deleting_sources()
     {
        for future in [false, true] {
            let (_root, pool, profile) = removal_fixture().await;
            let row = removal_record(&pool, "active", "first", false).await;
            if future {
                let mut envelope: Value = serde_json::from_slice(
                    &tokio::fs::read(&row.4).await.unwrap(),
                )
                .unwrap();
                envelope["schema_version"] = json!(99);
                let bytes = serde_json::to_vec(&envelope).unwrap();
                tokio::fs::write(&row.4, &bytes).await.unwrap();
                sqlx::query("UPDATE main.operation_publications SET sha256=? WHERE id='active'").bind(crate::state::recovery::record::digest(&bytes)).execute(&pool).await.unwrap();
            } else {
                tokio::fs::write(&row.4, b"broken primary").await.unwrap();
                tokio::fs::write(
                    format!("{}.previous", row.4),
                    b"broken fallback",
                )
                .await
                .unwrap();
            }
            assert!(
                preserve_instance_before_removal(&pool, "first", &profile)
                    .await
                    .is_err()
            );
            assert!(profile.is_dir());
            assert!(Path::new(&row.4).is_file());
            let after =
                removal_records(&mut *pool.acquire().await.unwrap(), "first")
                    .await
                    .unwrap()
                    .pop()
                    .unwrap();
            assert_eq!(after.4, row.4);
            assert_eq!(after.3, row.3);
            pool.close().await;
        }
    }

    #[test]
    fn clearing_known_optional_recovery_data_preserves_only_extensions() {
        let previous = json!({"rollback":{"content":{"files":["old"]},"future":{"retain":true}}});
        let known = json!({"rollback":{"content":{"files":["old"]}}});
        let mut incoming = json!({"rollback":{}});
        retain_extensions(&previous, &known, &mut incoming);
        assert!(incoming["rollback"].get("content").is_none());
        assert_eq!(incoming["rollback"]["future"], json!({"retain":true}));
    }
    async fn fixture() -> (tempfile::TempDir, sqlx::SqlitePool) {
        let root = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(root.path().join("data"))
            .await
            .unwrap();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(root.path().join("data/system.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        install(&mut *pool.acquire().await.unwrap()).await.unwrap();
        (root, pool)
    }
    #[tokio::test]
    async fn immutable_payload_is_invisible_until_its_core_decision_commits() {
        let (_root, pool) = fixture().await;
        {
            let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            write(&mut tx, "example", "one", Some(&json!({"count":1})))
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        write(&mut tx, "example", "one", Some(&json!({"count":2})))
            .await
            .unwrap();
        assert_eq!(
            read::<Value>(
                &mut *pool.acquire().await.unwrap(),
                "example",
                "one"
            )
            .await
            .unwrap()
            .unwrap()["count"],
            1
        );
        tx.rollback().await.unwrap();
        assert_eq!(
            read::<Value>(
                &mut *pool.acquire().await.unwrap(),
                "example",
                "one"
            )
            .await
            .unwrap()
            .unwrap()["count"],
            1
        );
    }
    #[tokio::test]
    async fn failed_record_is_local_and_cannot_be_consumed_as_an_empty_collection()
     {
        let (_root, pool) = fixture().await;
        let mut connection = pool.acquire().await.unwrap();
        write(&mut connection, "example", "bad", Some(&json!({"count":1})))
            .await
            .unwrap();
        write(
            &mut connection,
            "example",
            "good",
            Some(&json!({"count":2})),
        )
        .await
        .unwrap();
        let path:String=sqlx::query_scalar("SELECT object_path FROM main.operation_publications WHERE id='bad'").fetch_one(&mut *connection).await.unwrap();
        tokio::fs::write(&path, b"broken original").await.unwrap();
        tokio::fs::write(format!("{path}.previous"), b"broken backup")
            .await
            .unwrap();
        assert_eq!(
            all::<Value>(&mut connection, "example")
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            strict_all::<Value>(&mut connection, "example")
                .await
                .is_err()
        );
        assert!(
            write::<Value>(&mut connection, "example", "bad", None)
                .await
                .is_err()
        );
        assert_eq!(tokio::fs::read(path).await.unwrap(), b"broken original");
        assert_eq!(
            read::<Value>(&mut connection, "example", "good")
                .await
                .unwrap()
                .unwrap()["count"],
            2
        );
    }
    #[tokio::test]
    async fn future_payload_is_preserved_without_replay_or_deletion() {
        let (_root, pool) = fixture().await;
        let mut connection = pool.acquire().await.unwrap();
        write_imported(
            &mut connection,
            "example",
            "future",
            Some(&json!({"payload":"{\"schema_version\":2,\"future\":true}"})),
        )
        .await
        .unwrap();
        assert!(
            read::<Value>(&mut connection, "example", "future")
                .await
                .is_err()
        );
        assert!(
            write::<Value>(&mut connection, "example", "future", None)
                .await
                .is_err()
        );
        let deleted: bool = sqlx::query_scalar(
            "SELECT deleted FROM main.operation_publications WHERE id='future'",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert!(!deleted);
    }
    #[tokio::test]
    async fn external_instance_journal_never_creates_a_file_in_the_external_root()
     {
        let (root, pool) = fixture().await;
        let external = tempfile::tempdir().unwrap();
        let mut connection = pool.acquire().await.unwrap();
        connection
            .execute("ATTACH DATABASE ':memory:' AS instances")
            .await
            .unwrap();
        connection.execute("CREATE TABLE instances.instances(id TEXT,path TEXT,symlink_target TEXT,game_dir_override TEXT,linked_dot_minecraft TEXT)").await.unwrap();
        sqlx::query("INSERT INTO instances.instances VALUES('external','external',NULL,?,NULL)").bind(external.path().to_string_lossy().into_owned()).execute(&mut *connection).await.unwrap();
        write(
            &mut connection,
            "content_file_operations",
            "external",
            Some(&json!({"payload":"{}","committed":false})),
        )
        .await
        .unwrap();
        let path:String=sqlx::query_scalar("SELECT object_path FROM main.operation_publications WHERE id='external'").fetch_one(&mut *connection).await.unwrap();
        assert!(
            Path::new(&path).starts_with(
                tokio::fs::canonicalize(root.path())
                    .await
                    .unwrap()
                    .join(crate::state::layout::EXTERNAL_OPERATIONS)
            )
        );
        assert!(!external.path().join(".axolotl").exists());
    }
}

fn has_file_obligation(kind: &str, value: &Value) -> bool {
    match kind {
        "content_file_operations"
        | "resource_legacy_rollback_journals"
        | "resource_execution_journals" => true,
        "install_jobs" => {
            let pending = !matches!(
                value["status"].as_str(),
                Some("succeeded" | "failed" | "interrupted" | "canceled")
            );
            let state = value["state"]
                .as_str()
                .and_then(|state| serde_json::from_str::<Value>(state).ok());
            pending
                || state.is_none_or(|state| {
                    !state["rollback"]["content"].is_null()
                        || (!state["rollback"].is_null()
                            && !state["rollback_error"].is_null())
                })
        }
        "resource_history_cleanup_plans" => value["payload"]
            .as_str()
            .and_then(|payload| serde_json::from_str::<Value>(payload).ok())
            .is_none_or(|payload| {
                payload["moves"]
                    .as_array()
                    .is_some_and(|moves| !moves.is_empty())
            }),
        _ => false,
    }
}
/// A directory move may not strand a recovery payload with absolute paths.
/// This preflight runs before the first file transfer; retry the move after
/// pending recovery has completed. Old immutable generations remain retained.
pub(crate) async fn ensure_directory_move_supported(
    pool: &sqlx::SqlitePool,
    _old_content: &Path,
) -> crate::Result<()> {
    let mut connection = pool.acquire().await?;
    if !active(&mut connection).await? {
        return Ok(());
    }
    let keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT kind,id FROM main.operation_publications WHERE deleted=0",
    )
    .fetch_all(&mut *connection)
    .await?;
    for (kind, key) in keys {
        ensure_known_kind(&kind)?;
        let value: Value =
            read(&mut connection, &kind, &key).await?.ok_or_else(|| {
                invalid("Missing operation record prevents directory move")
            })?;
        if has_file_obligation(&kind, &value) {
            return Err(invalid(
                "Complete pending file recovery before moving the launcher directory",
            ));
        }
    }
    Ok(())
}

fn ensure_known_kind(kind: &str) -> crate::Result<()> {
    if !matches!(
        kind,
        "install_jobs"
            | "resource_operation_plans"
            | "resource_execution_journals"
            | "resource_history_cleanup_plans"
            | "resource_entry_graphs"
            | "content_update_jobs"
            | "content_file_operations"
            | "pending_backup_repository_cleanups"
            | "resource_legacy_rollback_journals"
            | "$legacy"
    ) {
        return Err(invalid(
            "Future operation kind preserved; reference cleanup is paused",
        ));
    }
    Ok(())
}
