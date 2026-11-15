//! JSON preferences projected into the shared, volatile configuration attachment.
//! A staged document is synced before its commit token enters the durable SQL
//! transaction. Startup finishes a committed stage before reading preferences.

use super::Collection;
use serde_json::{Map, Value, json};
use sqlx::{Executor, Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const DOCUMENTS: &[&str] = &[
    "resources",
    "sync",
    "internal",
    "proxy",
    "content_providers",
];

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

pub(super) async fn install_tracking(
    connection: &mut SqliteConnection,
    initialize: bool,
) -> Result<(), sqlx::Error> {
    if initialize {
        connection.execute("CREATE TABLE IF NOT EXISTS configuration.document_dirty (name TEXT PRIMARY KEY)").await?;
        connection.execute("CREATE TABLE IF NOT EXISTS configuration.document_removed_instances (id TEXT PRIMARY KEY)").await?;
        connection.execute("CREATE TABLE IF NOT EXISTS main.document_commits (id INTEGER PRIMARY KEY CHECK(id=1), token TEXT NOT NULL)").await?;
    }
    connection.execute("CREATE TEMP TRIGGER IF NOT EXISTS document_instance_removed AFTER DELETE ON instances.instances BEGIN INSERT INTO document_removed_instances SELECT OLD.id WHERE NOT EXISTS(SELECT 1 FROM document_removed_instances WHERE id=OLD.id); INSERT INTO document_dirty SELECT 'sync' WHERE NOT EXISTS(SELECT 1 FROM document_dirty WHERE name='sync'); END").await?;
    for (table, document) in [
        ("resource_preferences", "resources"),
        ("sync_feature_settings", "sync"),
        ("instance_sync_preferences", "sync"),
        ("settings", "internal"),
    ] {
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let statement = format!(
                "CREATE TEMP TRIGGER IF NOT EXISTS document_{table}_{event} AFTER {event} ON configuration.{table} BEGIN INSERT INTO document_dirty SELECT '{document}' WHERE NOT EXISTS(SELECT 1 FROM document_dirty WHERE name='{document}'); END"
            );
            connection
                .execute(sqlx::AssertSqlSafe(statement.as_str()))
                .await?;
        }
    }
    for (document, changed) in [
        (
            "proxy",
            "OLD.proxy_mode IS NOT NEW.proxy_mode OR OLD.proxy_url IS NOT NEW.proxy_url OR OLD.proxy_username IS NOT NEW.proxy_username OR OLD.proxy_password IS NOT NEW.proxy_password",
        ),
        (
            "content_providers",
            "OLD.curseforge_api_key IS NOT NEW.curseforge_api_key",
        ),
    ] {
        let statement = format!(
            "CREATE TEMP TRIGGER IF NOT EXISTS document_owned_{document} AFTER UPDATE ON configuration.settings WHEN {changed} BEGIN INSERT INTO document_dirty SELECT '{document}' WHERE NOT EXISTS(SELECT 1 FROM document_dirty WHERE name='{document}'); END"
        );
        connection
            .execute(sqlx::AssertSqlSafe(statement.as_str()))
            .await?;
    }
    Ok(())
}

async fn read_document(path: &Path) -> Result<Value, sqlx::Error> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(json!({"schema_version": 1, "data": {}}))
        }
        Err(error) => Err(error.into()),
    }
}

fn data(value: &mut Value) -> &mut Map<String, Value> {
    value
        .get_mut("data")
        .and_then(Value::as_object_mut)
        .expect("validated document")
}

fn object<'a>(
    value: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let item = value.entry(key.to_owned()).or_insert_with(|| json!({}));
    if !item.is_object() {
        *item = json!({});
    }
    item.as_object_mut().expect("object")
}

async fn snapshot(
    connection: &mut SqliteConnection,
    directory: &Path,
    names: &[String],
) -> Result<Vec<(String, Value)>, sqlx::Error> {
    let mut documents = Vec::new();
    if names.iter().any(|name| name == "resources") {
        let mut resources =
            read_document(&directory.join("resources.json")).await?;
        let source: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences WHERE id=1",
        )
        .fetch_one(&mut *connection)
        .await?;
        data(&mut resources)
            .insert("preferred_source".into(), Value::String(source));
        documents.push(("resources".into(), resources));
    }
    if names.iter().any(|name| name == "sync") {
        let mut sync = read_document(&directory.join("sync.json")).await?;
        let features: Vec<(String, bool, bool)> = sqlx::query_as("SELECT feature, globally_enabled, new_instance_default FROM sync_feature_settings").fetch_all(&mut *connection).await?;
        for (name, enabled, default) in features {
            let entry = object(object(data(&mut sync), "features"), &name);
            entry.insert("globally_enabled".into(), Value::Bool(enabled));
            entry.insert("new_instance_default".into(), Value::Bool(default));
        }
        let instances: Vec<(String, String, bool)> = sqlx::query_as("SELECT instance_id, feature, enabled FROM instance_sync_preferences").fetch_all(&mut *connection).await?;
        let unsupported =
            super::compatibility::unsupported_on(connection).await?;
        let known_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM instances")
                .fetch_all(&mut *connection)
                .await?;
        let removed_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM configuration.document_removed_instances",
        )
        .fetch_all(&mut *connection)
        .await?;
        let instance_map = object(data(&mut sync), "instances");
        for id in known_ids {
            if unsupported.contains(&id) {
                continue;
            }
            if let Some(value) =
                instance_map.get_mut(&id).and_then(Value::as_object_mut)
            {
                value.retain(|_, value| !value.is_boolean());
            }
        }
        for id in removed_ids {
            instance_map.remove(&id);
        }
        for (id, feature, enabled) in instances {
            if unsupported.contains(&id) {
                continue;
            }
            object(instance_map, &id).insert(feature, Value::Bool(enabled));
        }
        documents.push(("sync".into(), sync));
    }
    if names.iter().any(|name| name == "internal") {
        let mut internal =
            read_document(&directory.join("internal.json")).await?;
        let ads: bool = sqlx::query_scalar(
            "SELECT personalized_ads FROM settings WHERE id=0",
        )
        .fetch_one(&mut *connection)
        .await?;
        data(&mut internal).insert("personalized_ads".into(), Value::Bool(ads));
        documents.push(("internal".into(), internal));
    }
    if names.iter().any(|name| name == "proxy") {
        let mut proxy = read_document(&directory.join("proxy.json")).await?;
        let (mode, url, username, password): (String, String, String, String) = sqlx::query_as("SELECT proxy_mode,proxy_url,proxy_username,proxy_password FROM settings WHERE id=0").fetch_one(&mut *connection).await?;
        for (name, value) in [
            ("mode", mode),
            ("url", url),
            ("username", username),
            ("password", password),
        ] {
            data(&mut proxy).insert(name.into(), Value::String(value));
        }
        documents.push(("proxy".into(), proxy));
    }
    if names.iter().any(|name| name == "content_providers") {
        let mut providers =
            read_document(&directory.join("content_providers.json")).await?;
        let key: String = sqlx::query_scalar(
            "SELECT curseforge_api_key FROM settings WHERE id=0",
        )
        .fetch_one(&mut *connection)
        .await?;
        data(&mut providers)
            .insert("curseforge_api_key".into(), Value::String(key));
        documents.push(("content_providers".into(), providers));
    }
    Ok(documents)
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

async fn prepare(
    connection: &mut SqliteConnection,
    root: &Path,
) -> Result<Option<String>, sqlx::Error> {
    prepare_settings(connection, root, None, None).await
}

async fn prepare_settings(
    connection: &mut SqliteConnection,
    root: &Path,
    changed: Option<Option<&BTreeSet<String>>>,
    pool: Option<&SqlitePool>,
) -> Result<Option<String>, sqlx::Error> {
    // A previous SQL commit may have outlived publication. Publish its stage
    // inside the current write reservation before superseding its token.
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT token FROM main.document_commits WHERE id=1",
    )
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(previous) = previous {
        publish(root, &previous).await?;
        connection
            .execute("DELETE FROM main.document_commits WHERE id=1")
            .await?;
    }
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM configuration.document_dirty")
            .fetch_all(&mut *connection)
            .await?;
    if names.is_empty() && changed.is_none() {
        return Ok(None);
    }
    let directory = root.join("settings");
    let mut documents = snapshot(connection, &directory, &names).await?;
    if let Some(changed) = changed {
        let record = super::settings::Settings::read(
            connection,
            &super::settings::Singleton,
        )
        .await
        .map_err(protocol)?
        .ok_or_else(|| protocol("The settings projection is missing"))?;
        let pool = pool.ok_or_else(|| {
            protocol("A settings write requires its storage handle")
        })?;
        let source =
            crate::state::documents::Documents::at(directory.clone(), pool)
                .with_config_root(crate::state::documents::config_root());
        documents.extend(
            crate::state::documents::settings::staged_row_documents(
                &source,
                crate::state::Settings::from_record(record),
                changed,
            )
            .await
            .map_err(protocol)?,
        );
    }
    let token = uuid::Uuid::new_v4().to_string();
    let stage = directory.join(".transactions").join(&token);
    tokio::fs::create_dir_all(&stage).await?;
    let names = documents
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    for (name, value) in documents {
        sync_write(&stage.join(format!("{name}.json")), &value).await?;
    }
    sync_write(&stage.join("manifest.json"), &json!(names)).await?;
    sync_directory(&stage).await?;
    sync_directory(&directory.join(".transactions")).await?;
    sync_directory(&directory).await?;
    sqlx::query("INSERT INTO main.document_commits(id,token) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET token=excluded.token")
        .bind(&token).execute(&mut *connection).await?;
    connection
        .execute("DELETE FROM configuration.document_dirty")
        .await?;
    connection
        .execute("DELETE FROM configuration.document_removed_instances")
        .await?;
    Ok(Some(token))
}

async fn publish(root: &Path, token: &str) -> Result<(), sqlx::Error> {
    uuid::Uuid::parse_str(token).map_err(protocol)?;
    let directory = root.join("settings");
    let stage = directory.join(".transactions").join(token);
    if !tokio::fs::try_exists(&stage).await? {
        return Err(protocol("The committed preference stage is missing"));
    }
    let names: Vec<String> = serde_json::from_slice(
        &tokio::fs::read(stage.join("manifest.json")).await?,
    )
    .map_err(protocol)?;
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
        let bytes = tokio::fs::read(&source).await?;
        let _ = value;
        documents.push((name, bytes));
    }
    for (name, bytes) in documents {
        let target = directory.join(format!("{name}.json"));
        crate::util::io::write(&target, bytes)
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
    connection.execute("BEGIN IMMEDIATE").await?;
    let result = async {
        let current: Option<String> = sqlx::query_scalar(
            "SELECT token FROM main.document_commits WHERE id=1",
        )
        .fetch_optional(&mut *connection)
        .await?;
        if current.as_deref() != Some(token) {
            return Ok(());
        }
        publish(root, token).await?;
        connection
            .execute("DELETE FROM main.document_commits WHERE id=1")
            .await?;
        Ok::<(), sqlx::Error>(())
    }
    .await;
    match result {
        Ok(()) => {
            connection.execute("COMMIT").await?;
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            return Err(error);
        }
    }
    let stage = root.join("settings").join(".transactions").join(token);
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
    commit_settings(transaction, pool, false, None).await
}

pub(super) async fn commit_settings(
    mut transaction: Transaction<'_, Sqlite>,
    pool: &SqlitePool,
    settings: bool,
    changed: Option<&BTreeSet<String>>,
) -> Result<(), sqlx::Error> {
    let Some(root) = root(&mut transaction).await? else {
        return transaction.commit().await;
    };
    let token = prepare_settings(
        &mut transaction,
        &root,
        settings.then_some(changed),
        Some(pool),
    )
    .await?;
    transaction.commit().await?;
    if token.is_some() {
        // The token and synced stage are already durable. Publication failure
        // is retried at startup rather than reported as a failed SQL commit.
        if let Err(error) = flush_committed(pool).await {
            tracing::warn!(%error, "Preference publication deferred until restart");
        }
    }
    Ok(())
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

/// Restore the volatile row from the JSON authority before row-based callers
/// read or patch it. This projection never writes the documents back.
pub(super) async fn hydrate_settings(
    pool: &SqlitePool,
    root: &Path,
) -> crate::Result<()> {
    let documents =
        crate::state::documents::Documents::at(root.join("settings"), pool)
            .with_config_root(crate::state::documents::config_root());
    let mut settings = crate::state::documents::settings::read(
        &documents,
        &super::Handle::new(pool.clone()),
    )
    .await?;
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    if let Some(id) = settings.minimal_home_instance_id.as_deref() {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE id=?)",
        )
        .bind(id)
        .fetch_one(&mut *transaction)
        .await?;
        if !exists {
            // Before legacy import, the referenced instance is not present yet.
            // Keep the JSON choice so hydration can restore it after import.
            settings.minimal_home_instance_id = None;
        }
    }
    super::settings::Settings::write(
        &mut transaction,
        &super::settings::Singleton,
        Some(&settings.to_record()?),
    )
    .await?;
    transaction
        .execute("DELETE FROM configuration.document_dirty")
        .await?;
    transaction.commit().await?;
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
    let directory = root.join("settings");
    let resources = read_document(&directory.join("resources.json")).await?;
    if let Some(source) = resources["data"]["preferred_source"].as_str() {
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source=? WHERE id=1",
        )
        .bind(source)
        .execute(&mut *connection)
        .await?;
    }
    let sync = read_document(&directory.join("sync.json")).await?;
    if let Some(features) = sync["data"]["features"].as_object() {
        for (name, value) in features {
            if let (Some(enabled), Some(default)) = (
                value["globally_enabled"].as_bool(),
                value["new_instance_default"].as_bool(),
            ) {
                sqlx::query("INSERT INTO sync_feature_settings VALUES(?,?,?) ON CONFLICT(feature) DO UPDATE SET globally_enabled=excluded.globally_enabled,new_instance_default=excluded.new_instance_default")
                    .bind(name).bind(enabled).bind(default).execute(&mut *connection).await?;
            }
        }
    }
    let unsupported = super::compatibility::unsupported_on(connection).await?;
    if let Some(instances) = sync["data"]["instances"].as_object() {
        for (id, values) in instances {
            if unsupported.contains(id) {
                continue;
            }
            if let Some(values) = values.as_object() {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM instances WHERE id=?)",
                )
                .bind(id)
                .fetch_one(&mut *connection)
                .await?;
                if !exists {
                    continue;
                }
                for (feature, enabled) in values {
                    if let Some(enabled) = enabled.as_bool() {
                        sqlx::query("INSERT OR IGNORE INTO sync_feature_settings VALUES(?,0,0)").bind(feature).execute(&mut *connection).await?;
                        sqlx::query("INSERT INTO instance_sync_preferences VALUES(?,?,?) ON CONFLICT(instance_id,feature) DO UPDATE SET enabled=excluded.enabled")
                            .bind(id).bind(feature).bind(enabled).execute(&mut *connection).await?;
                    }
                }
            }
        }
    }
    let internal = read_document(&directory.join("internal.json")).await?;
    if let Some(ads) = internal["data"]["personalized_ads"].as_bool() {
        sqlx::query("UPDATE settings SET personalized_ads=? WHERE id=0")
            .bind(ads)
            .execute(&mut *connection)
            .await?;
    }
    let proxy = read_document(&directory.join("proxy.json")).await?;
    for (name, column) in [
        ("mode", "proxy_mode"),
        ("url", "proxy_url"),
        ("username", "proxy_username"),
        ("password", "proxy_password"),
    ] {
        if let Some(value) = proxy["data"][name].as_str() {
            let statement =
                format!("UPDATE settings SET {column}=? WHERE id=0");
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .bind(value)
                .execute(&mut *connection)
                .await?;
        }
    }
    let providers =
        read_document(&directory.join("content_providers.json")).await?;
    if let Some(value) = providers["data"].get("curseforge_api_key") {
        if value.is_null() || value.is_string() {
            sqlx::query("UPDATE settings SET curseforge_api_key=? WHERE id=0")
                .bind(value.as_str().unwrap_or_default())
                .execute(&mut *connection)
                .await?;
        }
    }
    connection
        .execute("DELETE FROM configuration.document_dirty")
        .await?;
    connection
        .execute("DELETE FROM configuration.document_removed_instances")
        .await?;
    Ok(())
}

pub(super) async fn hand_over(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let Some(root) = root(&mut transaction).await? else {
        return Ok(());
    };
    for name in DOCUMENTS {
        if !tokio::fs::try_exists(
            root.join("settings").join(format!("{name}.json")),
        )
        .await?
        {
            sqlx::query(
                "INSERT OR IGNORE INTO configuration.document_dirty VALUES(?)",
            )
            .bind(name)
            .execute(&mut *transaction)
            .await?;
        }
    }
    prepare(&mut transaction, &root).await?;
    transaction.commit().await?;
    flush_committed(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::store::databases;

    #[tokio::test]
    async fn recovery_publishes_unknown_domains_without_interpreting_their_fields()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let token = uuid::Uuid::new_v4().to_string();
        let stage =
            temporary.path().join("settings/.transactions").join(&token);
        tokio::fs::create_dir_all(&stage).await.unwrap();
        let payload =
            json!({"schema_version":999,"data":{"future":{"opaque":[1,2,3]}}});
        sync_write(&stage.join("future_domain.json"), &payload)
            .await
            .unwrap();
        sync_write(&stage.join("manifest.json"), &json!(["future_domain"]))
            .await
            .unwrap();
        sqlx::query("INSERT INTO main.document_commits VALUES(1,?)")
            .bind(&token)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        assert_eq!(
            read_document(
                &temporary.path().join("settings/future_domain.json")
            )
            .await
            .unwrap(),
            payload
        );
        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM main.document_commits")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn startup_defers_home_instance_reference_until_legacy_import_finishes()
     {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        let original = br#"{"schema_version":2,"data":{"minimal_home_instance_id":"instance","future":{"keep":true}}}"#;
        tokio::fs::write(directory.join("appearance.json"), original)
            .await
            .unwrap();
        let source = temporary.path().join("legacy.db");
        let legacy = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        instance(&legacy).await;
        legacy.close().await;

        let pool = databases::open(temporary.path()).await.unwrap();
        let home: Option<String> = sqlx::query_scalar(
            "SELECT minimal_home_instance_id FROM settings WHERE id=0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(home, None);
        assert_eq!(
            tokio::fs::read(directory.join("appearance.json"))
                .await
                .unwrap(),
            original
        );

        databases::import(&pool, &source, temporary.path())
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let home: Option<String> = sqlx::query_scalar(
            "SELECT minimal_home_instance_id FROM settings WHERE id=0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(home.as_deref(), Some("instance"));
        assert_eq!(
            tokio::fs::read(directory.join("appearance.json"))
                .await
                .unwrap(),
            original
        );
    }

    #[tokio::test]
    async fn startup_keeps_home_instance_missing_from_the_current_database() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        pool.close().await;
        drop(pool);
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        let original = br#"{"schema_version":2,"data":{"minimal_home_instance_id":"another-channel","future":{"keep":true}}}"#;
        tokio::fs::write(directory.join("appearance.json"), original)
            .await
            .unwrap();

        let pool = databases::open(temporary.path()).await.unwrap();
        let home: Option<String> = sqlx::query_scalar(
            "SELECT minimal_home_instance_id FROM settings WHERE id=0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(home, None);
        assert_eq!(
            tokio::fs::read(directory.join("appearance.json"))
                .await
                .unwrap(),
            original
        );
    }

    #[tokio::test]
    async fn settings_projection_and_patch_keep_document_siblings_after_restart()
     {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        tokio::fs::write(directory.join("appearance.json"), br#"{"schema_version":2,"data":{"custom_window_title_enabled":true,"default_window_title":"Preserved title","future":{"keep":true}}}"#).await.unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        crate::state::documents::settings::identify(temporary.path());
        let handle = crate::state::store::Handle::new(pool.clone());
        let row = crate::state::Settings::read_row(&handle).await.unwrap();
        assert_eq!(row.default_window_title, "Preserved title");
        crate::api::settings::patch_with(&handle, json!({"locale":"zh-CN"}))
            .await
            .unwrap();
        drop(handle);
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let row = crate::state::Settings::read_row(
            &crate::state::store::Handle::new(pool.clone()),
        )
        .await
        .unwrap();
        assert_eq!(row.default_window_title, "Preserved title");
        assert_eq!(row.locale, "zh-CN");
        let document = read_document(&directory.join("appearance.json"))
            .await
            .unwrap();
        assert_eq!(document["data"]["future"]["keep"], true);
    }

    #[tokio::test]
    async fn committed_settings_stage_survives_loss_of_the_memory_projection() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("UPDATE settings SET locale='zh-TW'")
            .execute(&mut *transaction)
            .await
            .unwrap();
        let changed = BTreeSet::from(["locale".to_owned()]);
        prepare_settings(
            &mut transaction,
            temporary.path(),
            Some(Some(&changed)),
            Some(&pool),
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let row = crate::state::Settings::read_row(
            &crate::state::store::Handle::new(pool.clone()),
        )
        .await
        .unwrap();
        assert_eq!(row.locale, "zh-TW");
    }

    #[tokio::test]
    async fn a_whole_row_write_keeps_a_newer_document_and_does_not_block_startup()
     {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        let bytes = br#"{"schema_version":999,"data":{"locale":"zh-CN","future":true}}"#;
        tokio::fs::write(directory.join("appearance.json"), bytes)
            .await
            .unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        crate::state::documents::settings::identify(temporary.path());
        let handle = crate::state::store::Handle::new(pool.clone());
        let row = crate::state::Settings::read_row(&handle).await.unwrap();
        row.update(&handle).await.unwrap();
        assert_eq!(
            tokio::fs::read(directory.join("appearance.json"))
                .await
                .unwrap(),
            bytes
        );
    }

    async fn instance(pool: &SqlitePool) {
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES('instance','test','installed','none','test',0,0)")
            .execute(pool).await.unwrap();
    }

    #[tokio::test]
    async fn preferences_are_json_and_survive_reopen_with_unknown_fields() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        tokio::fs::write(directory.join("resources.json"), br#"{"schema_version":99,"data":{"preferred_source":"modrinth","future":{"keep":true}}}"#).await.unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        instance(&pool).await;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='curseforge'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query("UPDATE sync_feature_settings SET globally_enabled=1,new_instance_default=1 WHERE feature='screenshots'").execute(&mut *transaction).await.unwrap();
        sqlx::query("INSERT INTO instance_sync_preferences VALUES('instance','screenshots',1)").execute(&mut *transaction).await.unwrap();
        sqlx::query("UPDATE settings SET personalized_ads=0")
            .execute(&mut *transaction)
            .await
            .unwrap();
        commit(transaction, &pool).await.unwrap();
        let document = read_document(&directory.join("resources.json"))
            .await
            .unwrap();
        assert_eq!(document["schema_version"], 99);
        assert_eq!(document["data"]["future"]["keep"], true);
        assert_eq!(document["data"]["preferred_source"], "curseforge");
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let source: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source, "curseforge");
        let preference: bool = sqlx::query_scalar("SELECT enabled FROM instance_sync_preferences WHERE instance_id='instance' AND feature='screenshots'").fetch_one(&pool).await.unwrap();
        assert!(preference);
        let ads: bool =
            sqlx::query_scalar("SELECT personalized_ads FROM settings")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!ads);
        let mut connection = pool.acquire().await.unwrap();
        for name in [
            "resource_preferences",
            "sync_feature_settings",
            "instance_sync_preferences",
        ] {
            for domain in ["instances", "sync", "main"] {
                let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {domain}.sqlite_schema WHERE type='table' AND name=?"))).bind(name).fetch_one(&mut *connection).await.unwrap();
                assert_eq!(count, 0);
            }
        }
    }

    #[tokio::test]
    async fn startup_publishes_committed_stage_and_ignores_rolled_back_stage() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='curseforge'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        prepare(&mut transaction, temporary.path()).await.unwrap();
        transaction.commit().await.unwrap();
        assert!(!temporary.path().join("settings/resources.json").exists());
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let source: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source, "curseforge");
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='mcarchive'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        prepare(&mut transaction, temporary.path()).await.unwrap();
        transaction.rollback().await.unwrap();
        pool.close().await;
        drop(pool);
        let pool = databases::open(temporary.path()).await.unwrap();
        let source: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source, "curseforge");
    }

    #[tokio::test]
    async fn missing_committed_stage_member_preserves_the_recovery_token() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='curseforge'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        let token = prepare(&mut transaction, temporary.path())
            .await
            .unwrap()
            .unwrap();
        transaction.commit().await.unwrap();
        tokio::fs::remove_file(
            temporary
                .path()
                .join("settings/.transactions")
                .join(&token)
                .join("resources.json"),
        )
        .await
        .unwrap();
        assert!(flush_committed(&pool).await.is_err());
        let stored: String = sqlx::query_scalar(
            "SELECT token FROM main.document_commits WHERE id=1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored, token);
        assert!(!temporary.path().join("settings/resources.json").exists());
    }

    #[tokio::test]
    async fn completed_stages_do_not_overwrite_later_manual_edits() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='curseforge'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        commit(transaction, &pool).await.unwrap();
        pool.close().await;
        drop(pool);
        tokio::fs::write(
            temporary.path().join("settings/resources.json"),
            br#"{"schema_version":1,"data":{"preferred_source":"mcarchive"}}"#,
        )
        .await
        .unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let source: String = sqlx::query_scalar(
            "SELECT preferred_source FROM resource_preferences",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(source, "mcarchive");
    }
    #[tokio::test]
    async fn snapshot_finishes_committed_preferences_without_dangling_stage_tokens()
     {
        use sqlx::Connection;
        let temporary = tempfile::tempdir().unwrap();
        let pool = databases::open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query(
            "UPDATE resource_preferences SET preferred_source='curseforge'",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        prepare(&mut transaction, temporary.path()).await.unwrap();
        transaction.commit().await.unwrap();
        let snapshot =
            databases::backup(&pool, temporary.path()).await.unwrap();
        let mut copy = SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(snapshot.join("system.db"))
                .read_only(true),
        )
        .await
        .unwrap();
        let tokens: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM document_commits")
                .fetch_one(&mut copy)
                .await
                .unwrap();
        assert_eq!(tokens, 0);
        copy.close().await.unwrap();
        let document: Value = serde_json::from_slice(
            &tokio::fs::read(snapshot.join("settings/resources.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(document["data"]["preferred_source"], "curseforge");
    }
}
