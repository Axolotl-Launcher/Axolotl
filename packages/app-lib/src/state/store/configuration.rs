//! Configuration publication decisions. Legacy SQL adapters remain import-only.
//! A staged document is synced before its commit token enters the durable SQL
//! transaction. Startup finishes a committed stage before reading preferences.

use crate::state::layout;
use serde_json::{Value, json};
use sqlx::{
    Connection, Executor, Sqlite, SqliteConnection, SqlitePool, Transaction,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn protocol(error: impl std::fmt::Display) -> sqlx::Error {
    sqlx::Error::Protocol(error.to_string())
}

async fn root(
    connection: &mut SqliteConnection,
) -> Result<Option<PathBuf>, sqlx::Error> {
    let file: String = sqlx::query_scalar(
        "SELECT file FROM pragma_database_list WHERE name = 'main'",
    )
    .fetch_one(connection)
    .await?;
    let path = Path::new(&file);
    Ok(
        (path.file_name().and_then(|name| name.to_str()) == Some("system.db"))
            .then(|| {
                path.parent().and_then(Path::parent).map(Path::to_path_buf)
            })
            .flatten(),
    )
}

async fn read_document(path: &Path) -> Result<Value, sqlx::Error> {
    if !tokio::fs::try_exists(path).await? {
        return Ok(json!({"schema_version":1,"data":{}}));
    }
    match bounded_bytes(path, 1 << 20).await {
        Ok(bytes) => {
            if bytes.len() > 1 << 20 {
                return Err(protocol(
                    "Configuration document exceeds its bound",
                ));
            }
            let value: Value =
                serde_json::from_slice(&bytes).map_err(protocol)?;
            if !value.get("data").is_some_and(Value::is_object) {
                return Err(protocol(format!(
                    "Invalid preference document {}",
                    path.display()
                )));
            }
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

async fn sync_write(path: &Path, value: &Value) -> Result<(), sqlx::Error> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .await?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(protocol)?)
        .await?;
    file.sync_all().await?;
    Ok(())
}

async fn publish(root: &Path, token: &str) -> Result<(), sqlx::Error> {
    uuid::Uuid::parse_str(token).map_err(protocol)?;
    let directory = root.join(layout::SETTINGS);
    let stage = directory.join(".transactions").join(token);
    if !tokio::fs::try_exists(&stage).await? {
        return Err(protocol("The committed preference stage is missing"));
    }
    if tokio::fs::try_exists(stage.join("published")).await? {
        if bounded_bytes(&stage.join("published"), 64).await?
            == token.as_bytes()
        {
            return Ok(());
        }
        return Err(protocol("Invalid configuration publication receipt"));
    }
    let names: Vec<String> = serde_json::from_slice(
        &bounded_bytes(&stage.join("manifest.json"), 1 << 20).await?,
    )
    .map_err(protocol)?;
    if names.len() > 256 {
        return Err(protocol("Configuration stage contains too many domains"));
    }
    let mut documents = Vec::new();
    for name in names {
        // A future configuration domain is opaque but can still be published
        // byte for byte. Only the path shape is part of the recovery protocol.
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'-')
            })
        {
            return Err(protocol("Invalid staged preference document name"));
        }
        let source = stage.join(format!("{name}.json"));
        let value = read_document(&source).await?;
        // Unlike an ordinary absent document, a missing member of a committed
        // stage is an incomplete recovery record and cannot be defaulted.
        let bytes = bounded_bytes(&source, 1 << 20).await?;
        let _ = value;
        documents.push((name, bytes));
    }
    for (name, bytes) in documents {
        let target = directory.join(format!("{name}.json"));
        crate::state::persistence::document::write_json(&target, bytes)
            .await
            .map_err(protocol)?;
        tokio::fs::OpenOptions::new()
            .write(true)
            .open(target)
            .await?
            .sync_all()
            .await?;
    }
    sync_directory(&directory).await?;
    let mut receipt = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.join("published"))
        .await?;
    use tokio::io::AsyncWriteExt;
    receipt.write_all(token.as_bytes()).await?;
    receipt.sync_all().await?;
    sync_directory(&stage).await?;
    Ok(())
}

async fn sync_directory(path: &Path) -> Result<(), sqlx::Error> {
    #[cfg(unix)]
    tokio::fs::File::open(path).await?.sync_all().await?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn finalize(
    connection: &mut SqliteConnection,
    root: &Path,
    token: &str,
) -> Result<(), sqlx::Error> {
    uuid::Uuid::parse_str(token).map_err(protocol)?;
    // A failed SQLite COMMIT can leave the transaction active. Use a managed
    // transaction so both errors and cancellation roll it back before reuse.
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    {
        let _writing = crate::state::documents::WRITES.lock().await;
        let _files = super::configuration_documents::lock_files(root)
            .await
            .map_err(protocol)?;
        let current: Option<String> = sqlx::query_scalar(
            "SELECT token FROM main.document_commits WHERE id=1",
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if current.as_deref() == Some(token) {
            publish(root, token).await?;
            transaction
                .execute("DELETE FROM main.document_commits WHERE id=1")
                .await?;
        }
    }
    transaction.commit().await?;
    let stage = root
        .join(layout::SETTINGS)
        .join(".transactions")
        .join(token);
    if let Err(error) = tokio::fs::remove_dir_all(&stage).await {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(%error, "Completed preference stage cleanup deferred");
        }
    }
    Ok(())
}

/// Commit either a legacy transaction or the attached database transaction.
/// Configuration values live only in synced JSON stages, never in main.db.
pub(crate) async fn commit(
    transaction: Transaction<'_, Sqlite>,
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    transaction.commit().await?;
    if super::configuration_documents::runtime(pool) {
        flush_committed(pool).await?;
    }
    Ok(())
}

pub(super) async fn commit_settings(
    transaction: Transaction<'_, Sqlite>,
    _pool: &SqlitePool,
    _settings: bool,
    _changed: Option<&BTreeSet<String>>,
) -> Result<(), sqlx::Error> {
    transaction.commit().await
}

pub(crate) async fn flush_committed(
    pool: &SqlitePool,
) -> Result<(), sqlx::Error> {
    let mut connection = pool.acquire().await?;
    let Some(root) = root(&mut connection).await? else {
        return Ok(());
    };
    let token: Option<String> = sqlx::query_scalar(
        "SELECT token FROM main.document_commits WHERE id=1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(token) = token {
        finalize(&mut connection, &root, &token).await?;
    }
    Ok(())
}

pub(super) async fn recover_and_load(
    connection: &mut SqliteConnection,
    root: &Path,
) -> Result<(), sqlx::Error> {
    let token: Option<String> = sqlx::query_scalar(
        "SELECT token FROM main.document_commits WHERE id=1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(token) = token {
        finalize(connection, root, &token).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn busy_configuration_commit_releases_the_writer_and_can_be_retried()
    {
        let root = tempfile::tempdir().unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        let mut publication = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let token = prepare_documents(
            &mut publication,
            root.path(),
            vec![(
                "resources".into(),
                json!({"schema_version":1,"data":{"preferred_source":"curseforge"}}),
            )],
        )
        .await
        .unwrap()
        .unwrap();
        publication.commit().await.unwrap();

        let mut writer = pool.acquire().await.unwrap();
        writer.execute("PRAGMA busy_timeout=0").await.unwrap();
        let mut reader = pool.begin().await.unwrap();
        assert_eq!(
            pending_token(&mut reader).await.unwrap().as_deref(),
            Some(token.as_str())
        );
        let error = finalize(&mut writer, root.path(), &token)
            .await
            .unwrap_err();
        assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("5")
        );
        reader.rollback().await.unwrap();

        let mut probe = writer.begin_with("BEGIN IMMEDIATE").await.unwrap();
        assert_eq!(
            pending_token(&mut probe).await.unwrap().as_deref(),
            Some(token.as_str())
        );
        probe.rollback().await.unwrap();
        finalize(&mut writer, root.path(), &token).await.unwrap();
        assert_eq!(pending_token(&mut writer).await.unwrap(), None);
        let preferences =
            read_document(&root.path().join("data/settings/resources.json"))
                .await
                .unwrap();
        assert_eq!(preferences["data"]["preferred_source"], "curseforge");
    }

    #[tokio::test]
    async fn staged_document_is_invisible_until_decision_and_replay_does_not_overwrite_a_manual_edit()
     {
        let root = tempfile::tempdir().unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let token=prepare_documents(&mut transaction,root.path(),vec![("resources".into(),json!({"schema_version":1,"data":{"preferred_source":"curseforge","future":{"keep":true}}}))]).await.unwrap().unwrap();
        assert!(!root.path().join("data/settings/resources.json").exists());
        transaction.commit().await.unwrap();
        publish_prepared(root.path(), &token).await.unwrap();
        let manual = json!({"schema_version":1,"data":{"preferred_source":"mcarchive","future":{"keep":true}}});
        crate::state::persistence::document::write_json(
            &root.path().join("data/settings/resources.json"),
            serde_json::to_vec(&manual).unwrap(),
        )
        .await
        .unwrap();
        // Simulate a settings read that publishes then rolls its SQL token deletion back.
        for _ in 0..2 {
            let mut transaction =
                pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            publish_committed_in(&mut transaction, root.path())
                .await
                .unwrap();
            transaction.rollback().await.unwrap();
        }
        assert_eq!(
            read_document(&root.path().join("data/settings/resources.json"))
                .await
                .unwrap(),
            manual
        );
        flush_committed(&pool).await.unwrap();
    }
    #[tokio::test]
    async fn aborted_configuration_unit_preserves_unknown_siblings_and_never_publishes()
     {
        let root = tempfile::tempdir().unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        let handle = super::super::Handle::new(pool.clone());
        let path = root.path().join("data/settings/resources.json");
        let original = json!({"schema_version":1,"future_envelope":7,"data":{"preferred_source":"modrinth","unknown":true}});
        crate::state::persistence::document::write_json(
            &path,
            serde_json::to_vec(&original).unwrap(),
        )
        .await
        .unwrap();
        {
            let mut unit = super::super::atom(&handle).await.unwrap();
            super::super::configuration_documents::payload(
                unit.configuration()
                    .await
                    .unwrap()
                    .document("resources")
                    .await
                    .unwrap(),
            )
            .unwrap()
            .insert("preferred_source".into(), json!("curseforge"));
        }
        assert_eq!(read_document(&path).await.unwrap(), original);
        super::super::configuration_documents::set_preferred_source(
            &handle,
            "curseforge",
        )
        .await
        .unwrap();
        let saved = read_document(&path).await.unwrap();
        assert_eq!(saved["future_envelope"], 7);
        assert_eq!(saved["data"]["unknown"], true);
        assert_eq!(saved["data"]["preferred_source"], "curseforge");
        let names: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_database_list")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(!names.iter().any(|name| name == "configuration"));
    }
    #[tokio::test]
    async fn newer_or_corrupt_domain_refuses_mutation_without_damaging_other_documents()
     {
        let root = tempfile::tempdir().unwrap();
        let pool = super::super::databases::open(root.path()).await.unwrap();
        let handle = super::super::Handle::new(pool);
        let path = root.path().join("data/settings/resources.json");
        let newer = br#"{"schema_version":9,"data":{"future":[1,2]}}"#;
        crate::state::persistence::document::write_json(&path, newer.to_vec())
            .await
            .unwrap();
        assert!(
            super::super::configuration_documents::set_preferred_source(
                &handle,
                "curseforge"
            )
            .await
            .is_err()
        );
        assert_eq!(tokio::fs::read(path).await.unwrap(), newer);
    }
}

/// Complete an already committed settings decision under the caller's core
/// reservation. No nested pool acquisition is allowed while sync owns its lock.
pub(super) async fn publish_committed_in(
    connection: &mut SqliteConnection,
    root: &Path,
) -> Result<(), sqlx::Error> {
    let token: Option<String> = sqlx::query_scalar(
        "SELECT token FROM main.document_commits WHERE id=1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(token) = token {
        publish(root, &token).await?;
        sqlx::query("DELETE FROM main.document_commits WHERE id=1 AND token=?")
            .bind(token)
            .execute(connection)
            .await?;
    }
    Ok(())
}

/// The coordinator stores decisions only; settings themselves never become rows.
pub(crate) async fn install_decisions(
    connection: &mut SqliteConnection,
) -> Result<(), sqlx::Error> {
    connection.execute("CREATE TABLE IF NOT EXISTS main.document_commits (id INTEGER PRIMARY KEY CHECK(id=1), token TEXT NOT NULL)").await?;
    Ok(())
}
pub(crate) async fn prepare_documents(
    connection: &mut SqliteConnection,
    root: &Path,
    mut documents: Vec<(String, Value)>,
) -> Result<Option<String>, sqlx::Error> {
    if documents.is_empty() {
        return Ok(None);
    }
    if documents.len() > 256
        || documents.iter().any(|(_, value)| {
            serde_json::to_vec(value)
                .map_or(true, |bytes| bytes.len() > 1 << 20)
        })
    {
        return Err(protocol("Configuration stage exceeds its bounds"));
    }
    // Include an earlier stage from this transaction rather than publishing an
    // uncommitted decision. The next token carries every still-pending member.
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT token FROM main.document_commits WHERE id=1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(previous) = previous {
        uuid::Uuid::parse_str(&previous).map_err(protocol)?;
        let stage = root
            .join(layout::SETTINGS)
            .join(".transactions")
            .join(previous);
        let names: Vec<String> = serde_json::from_slice(
            &bounded_bytes(&stage.join("manifest.json"), 1 << 20).await?,
        )
        .map_err(protocol)?;
        for name in names {
            validate_name(&name)?;
            if !documents.iter().any(|(current, _)| current == &name) {
                documents.push((
                    name.clone(),
                    read_document(&stage.join(format!("{name}.json"))).await?,
                ));
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    let directory = root.join(layout::SETTINGS).join(".transactions");
    let stage = directory.join(&token);
    tokio::fs::create_dir_all(&stage).await?;
    for (name, value) in &documents {
        validate_name(name)?;
        sync_write(&stage.join(format!("{name}.json")), value).await?;
    }
    sync_write(
        &stage.join("manifest.json"),
        &json!(documents.iter().map(|(name, _)| name).collect::<Vec<_>>()),
    )
    .await?;
    sync_directory(&stage).await?;
    sync_directory(&directory).await?;
    sync_directory(&root.join(layout::SETTINGS)).await?;
    sqlx::query("INSERT INTO main.document_commits VALUES(1,?) ON CONFLICT(id) DO UPDATE SET token=excluded.token").bind(&token).execute(connection).await?;
    Ok(Some(token))
}
fn validate_name(name: &str) -> Result<(), sqlx::Error> {
    if name.is_empty()
        || name.len() > 64
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'-')
        })
    {
        return Err(protocol("Invalid configuration domain path"));
    }
    Ok(())
}
/// Called while the Atom still owns the settings writer. Do not reacquire it.
pub(crate) async fn publish_committed_in_pool(
    pool: &SqlitePool,
    root: &Path,
) -> crate::Result<()> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    publish_committed_in(&mut transaction, root).await?;
    transaction.commit().await?;
    Ok(())
}

/// Import only a disposable, historical all-in-one snapshot. Never attach its
/// configuration tables to the running core or overwrite an existing authority.
pub(crate) async fn import_legacy(
    source: &Path,
    root: &Path,
) -> crate::Result<()> {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(source)
                .foreign_keys(false),
        )
        .await?;
    let handle = super::Handle::new(pool.clone());
    crate::state::Settings::migrate_legacy_row(&handle).await?;
    if !crate::state::documents::hand_over(&pool, root).await {
        return Err(crate::ErrorKind::FSError(
            "Legacy configuration document handover is incomplete".into(),
        )
        .into());
    }
    let columns = super::settings::Settings::owned_columns(&handle).await?;
    let mut connection = pool.acquire().await?;
    // These collections are outside the Settings shape. Preserve all unknown
    // columns in the old source, which remains a retained migration attachment.
    let mut sync =
        json!({"schema_version":1,"data":{"features":{},"instances":{}}});
    for (name,enabled,default) in sqlx::query_as::<_,(String,bool,bool)>("SELECT feature,globally_enabled,new_instance_default FROM sync_feature_settings").fetch_all(&mut *connection).await? {
        sync["data"]["features"][&name]=json!({"globally_enabled":enabled,"new_instance_default":default});
    }
    for (id, feature, enabled) in sqlx::query_as::<_, (String, String, bool)>(
        "SELECT instance_id,feature,enabled FROM instance_sync_preferences",
    )
    .fetch_all(&mut *connection)
    .await?
    {
        sync["data"]["instances"][&id][&feature] = json!(enabled);
    }
    let resource: String = sqlx::query_scalar(
        "SELECT preferred_source FROM resource_preferences WHERE id=1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let ads: bool =
        sqlx::query_scalar("SELECT personalized_ads FROM settings WHERE id=0")
            .fetch_one(&mut *connection)
            .await?;
    for (name, value) in [
        ("sync", sync),
        (
            "resources",
            json!({"schema_version":1,"data":{"preferred_source":resource}}),
        ),
        (
            "internal",
            json!({"schema_version":1,"data":{"personalized_ads":ads}}),
        ),
        (
            "proxy",
            json!({"schema_version":1,"data":{"mode":columns.proxy_mode,"url":columns.proxy_url,"username":columns.proxy_username,"password":columns.proxy_password}}),
        ),
        (
            "content_providers",
            json!({"schema_version":1,"data":{"curseforge_api_key":columns.curseforge_api_key}}),
        ),
    ] {
        let target = root.join(layout::SETTINGS).join(format!("{name}.json"));
        if tokio::fs::try_exists(&target).await? {
            continue;
        }
        let mut value = value;
        let documents = crate::state::documents::Documents::at(
            root.join(layout::SETTINGS),
            &pool,
        )
        .with_config_root(crate::state::documents::config_root());
        match documents.shipped_keys(name, 1).await {
            crate::state::documents::Shipped::Keys(provided) => {
                value["data"]
                    .as_object_mut()
                    .expect("payload")
                    .retain(|key, _| !provided.contains_key(key));
            }
            crate::state::documents::Shipped::Missing => {}
            crate::state::documents::Shipped::Unreadable => {
                return Err(crate::ErrorKind::InputError(format!(
                    "Deployment configuration {name} unavailable during import"
                ))
                .into());
            }
        }
        crate::state::persistence::document::write_json(
            &target,
            serde_json::to_vec_pretty(&value)?,
        )
        .await?;
    }
    drop(connection);
    pool.close().await;
    Ok(())
}

pub(crate) async fn pending_token(
    connection: &mut SqliteConnection,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT token FROM main.document_commits WHERE id=1")
        .fetch_optional(connection)
        .await
}
pub(crate) async fn publish_prepared(
    root: &Path,
    token: &str,
) -> Result<(), sqlx::Error> {
    publish(root, token).await
}

pub(crate) async fn clear_published(
    pool: &SqlitePool,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM main.document_commits WHERE id=1 AND token=?")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

async fn bounded_bytes(path: &Path, max: u64) -> Result<Vec<u8>, sqlx::Error> {
    let metadata = tokio::fs::metadata(path).await?;
    if !metadata.is_file() || metadata.len() > max {
        return Err(protocol("Configuration journal member exceeds its bound"));
    }
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    tokio::fs::File::open(path)
        .await?
        .take(max + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > max {
        return Err(protocol("Configuration journal member exceeds its bound"));
    }
    Ok(bytes)
}
