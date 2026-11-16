//! Stable, attached database domains. Application release numbers never decide
//! whether these files may be opened. The frozen baseline is additive only;
//! historical SQLx migrations are used solely when importing the old app.db.
//!
//! A durable main database and rollback journals are intentional: SQLite's
//! super-journal makes transactions spanning attached files atomic. Changing
//! these connections to WAL would lose that property.

use serde::Deserialize;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
};
use sqlx::{Connection, Executor, SqliteConnection, SqlitePool};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

const DOMAINS: &[&str] = &[
    "instances",
    "accounts",
    "sync",
    "media",
    "operations",
    "storage",
    "records",
    "telemetry",
];

#[derive(Deserialize)]
struct Baseline {
    tables: Vec<Table>,
    objects: Vec<Object>,
}

#[derive(Deserialize)]
struct Table {
    name: String,
    domain: String,
    sql: String,
    columns: Vec<String>,
    foreign_keys: Vec<ForeignKey>,
    seeds: Vec<String>,
}

#[derive(Deserialize)]
struct ForeignKey {
    parent: String,
    columns: Vec<ForeignColumn>,
    on_delete: String,
    on_update: String,
}

#[derive(Deserialize)]
struct ForeignColumn {
    child: String,
    parent: String,
}

#[derive(Deserialize)]
struct Object {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

fn baseline() -> &'static Baseline {
    static VALUE: OnceLock<Baseline> = OnceLock::new();
    VALUE.get_or_init(|| {
        serde_json::from_str(include_str!("database_baseline.json"))
            .expect("the checked-in database baseline is valid")
    })
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn qualified(table: &Table) -> String {
    format!("{}.{}", quote(&table.domain), quote(&table.name))
}

pub(crate) fn directory(settings_dir: &Path) -> PathBuf {
    settings_dir.join("databases")
}

pub(crate) fn exists(settings_dir: &Path) -> bool {
    directory(settings_dir).join("system.db").is_file()
}

async fn attached_domains(
    connection: &mut SqliteConnection,
) -> Result<HashSet<String>, sqlx::Error> {
    Ok(
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_database_list",
        )
        .fetch_all(connection)
        .await?
        .into_iter()
        .collect(),
    )
}

/// Collection cleanup cannot infer the references held by a newer protocol.
/// Preserve files until a reader that understands those records is available.
pub(crate) async fn ensure_cleanup_supported(
    pool: &SqlitePool,
) -> crate::Result<()> {
    super::recovery_mode::ensure_writable(pool)?;
    if pool
        .connect_options()
        .get_filename()
        .file_name()
        .is_none_or(|name| name != "system.db")
    {
        return Ok(());
    }
    if !super::compatibility::unsupported_instances(pool)
        .await?
        .is_empty()
    {
        return Err(crate::ErrorKind::InputError(
            "Cleanup is paused because some instances use a newer data format"
                .into(),
        )
        .into());
    }
    if super::compatibility::unknown_recovery_payloads(pool).await? {
        return Err(crate::ErrorKind::InputError("Cleanup is paused because a newer recovery protocol may retain files".into()).into());
    }
    let mut connection = pool.acquire().await?;
    let attached = attached_domains(&mut connection).await?;
    let file = pool.connect_options().get_filename().to_path_buf();
    let root = file
        .parent()
        .and_then(Path::parent)
        .expect("domain data root");
    super::legacy_documents::ensure_cleanup_supported(root).await?;
    let shared = crate::state::sync_state::root(
        &crate::state::persistence::Persistence::new(root),
    )
    .await?;
    crate::state::sync_state::ensure_supported(&shared).await?;
    if DOMAINS
        .iter()
        .filter(|domain| {
            !matches!(
                **domain,
                "accounts" | "telemetry" | "media" | "records" | "sync"
            )
        })
        .any(|domain| !attached.contains(*domain))
    {
        return Err(crate::ErrorKind::InputError("Cleanup is paused until independent document references can be checked".into()).into());
    }
    for domain in attached {
        if !DOMAINS.contains(&domain.as_str()) {
            continue;
        }
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT COUNT(*) FROM {}.extension_records",
            quote(&domain)
        )))
        .fetch_one(pool)
        .await?;
        if count != 0 {
            return Err(crate::ErrorKind::InputError("Cleanup is paused because newer extension records may retain files".into()).into());
        }
    }
    Ok(())
}

pub(super) async fn cross_domain_issues(
    connection: &mut SqliteConnection,
) -> Result<usize, sqlx::Error> {
    let mut issues = 0;
    let attached = attached_domains(connection).await?;
    for table in &baseline().tables {
        if !attached.contains(&table.domain) {
            continue;
        }
        for foreign in &table.foreign_keys {
            let parent = baseline()
                .tables
                .iter()
                .find(|candidate| candidate.name == foreign.parent)
                .expect("foreign parent");
            if table.domain == parent.domain {
                continue;
            }
            if !attached.contains(&parent.domain) {
                continue;
            }
            let present = foreign
                .columns
                .iter()
                .map(|column| {
                    format!("child.{} IS NOT NULL", quote(&column.child))
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matching = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "parent.{} = child.{}",
                        quote(&column.parent),
                        quote(&column.child)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let statement = format!(
                "SELECT COUNT(*) FROM {} AS child WHERE {present} AND NOT EXISTS(SELECT 1 FROM {} AS parent WHERE {matching})",
                qualified(table),
                qualified(parent)
            );
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(statement))
                .fetch_one(&mut *connection)
                .await?;
            issues += count as usize;
        }
    }
    Ok(issues)
}

/// Reserve every domain before copying its committed rollback-journal image.
/// The final directory appears only after the complete set has been synced.
pub(crate) async fn backup(
    pool: &SqlitePool,
    settings_dir: &Path,
) -> crate::Result<PathBuf> {
    let parent = crate::state::db_backup::app_db_backup_dir_for(
        &directory(settings_dir).join("system.db"),
    )?;
    tokio::fs::create_dir_all(&parent).await?;
    let id = uuid::Uuid::new_v4();
    let staging = parent.join(format!(".{id}.staging"));
    let destination = parent.join(id.to_string());
    tokio::fs::create_dir(&staging).await?;
    // Ordinary settings writers take this lock before writing their JSON.
    // Keep their documents and the attached SQL image in the same snapshot.
    // sched-order: theseus::api::settings::SETTINGS_WRITE
    let _settings = crate::api::settings::SETTINGS_WRITE.lock().await;
    // sched-order: theseus::state::documents::WRITES
    let _documents = crate::state::documents::WRITES.lock().await;
    let mut transaction = loop {
        super::configuration::flush_committed(pool).await?;
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
        let pending: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM main.document_commits)",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if !pending {
            break transaction;
        }
        transaction.rollback().await?;
    };
    let result = async {
        let domains = attached_domains(&mut *transaction).await?;
        for name in std::iter::once("system").chain(
            DOMAINS
                .iter()
                .copied()
                .filter(|name| domains.contains(*name)),
        ) {
            let file = format!("{name}.db");
            let target = staging.join(&file);
            tokio::fs::copy(directory(settings_dir).join(&file), &target)
                .await?;
            tokio::fs::OpenOptions::new()
                .write(true)
                .open(&target)
                .await?
                .sync_all()
                .await?;
        }
        let settings = settings_dir.join("settings");
        let target_settings = staging.join("settings");
        tokio::fs::create_dir(&target_settings).await?;
        if tokio::fs::try_exists(&settings).await? {
            let mut entries = tokio::fs::read_dir(&settings).await?;
            while let Some(entry) = entries.next_entry().await? {
                if !entry.file_type().await?.is_file()
                    || entry.path().extension().and_then(|part| part.to_str())
                        != Some("json")
                {
                    continue;
                }
                let target = target_settings.join(entry.file_name());
                tokio::fs::copy(entry.path(), &target).await?;
                tokio::fs::OpenOptions::new()
                    .write(true)
                    .open(&target)
                    .await?
                    .sync_all()
                    .await?;
            }
        }
        // Keep takeover receipts with the snapshot. Restoring only the SQL
        // files must never make a retired domain active again.
        let receipts = settings_dir.join("state/persistence/imports");
        if tokio::fs::try_exists(&receipts).await? {
            let target = staging.join("state/persistence/imports");
            tokio::fs::create_dir_all(&target).await?;
            let mut entries = tokio::fs::read_dir(&receipts).await?;
            while let Some(entry) = entries.next_entry().await? {
                if entry.path().extension().and_then(|value| value.to_str())
                    != Some("json")
                {
                    continue;
                }
                if !entry.file_type().await?.is_file() {
                    return Err(crate::ErrorKind::FSError(
                        "Cannot snapshot a nonregular handover receipt".into(),
                    )
                    .into());
                }
                let name = entry
                    .path()
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "Invalid handover receipt name".into(),
                        )
                    })?;
                let snapshot = crate::state::persistence::import::snapshot(
                    &crate::state::persistence::Persistence::new(settings_dir), &name,
                ).await;
                let files = match snapshot {
                    Ok(files)=>files,
                    Err(error) if name=="daily-playtime"=>{
                        tracing::warn!(%error,"Core backup excludes unavailable optional daily history descriptor");
                        crate::state::persistence::document::write_json(&staging.join("backup-status.json"),serde_json::to_vec(&serde_json::json!({"optional_exclusions":["records.db","state/records.json"]}))?).await?;
                        continue;
                    }
                    Err(error)=>return Err(error),
                };
                for (relative,bytes) in files {
                    let destination = staging.join(relative);
                    if destination.extension().and_then(|value| value.to_str())
                        == Some("json")
                    {
                        crate::state::persistence::document::write_json(
                            &destination,
                            bytes,
                        )
                        .await?;
                    } else {
                        crate::state::persistence::document::write_blob(
                            &destination,
                            bytes,
                        )
                        .await?;
                    }
                }
            }
        }
        if let Err(error)=super::sync_documents::backup_in(&mut transaction,&crate::state::persistence::Persistence::new(settings_dir),&staging.join("synced-options")).await{
            tracing::warn!(%error,"Backup excludes unavailable shared sync state");
            crate::state::persistence::document::write_json(&staging.join("sync-backup-status.json"),serde_json::to_vec(&serde_json::json!({"unavailable":true}))?).await?;
        }
        if let Err(error) = super::records_db::backup(settings_dir, &staging.join("records.db")).await {
            tracing::warn!(%error,"Core snapshot excludes unavailable optional daily history");
            crate::state::persistence::document::write_json(&staging.join("backup-status.json"),serde_json::to_vec(&serde_json::json!({"optional_exclusions":["records.db"]}))?).await?;
        }
        crate::state::instances::manifest::backup(settings_dir,&staging).await?;
        tokio::fs::write(staging.join("complete"), b"axolotl database set\n")
            .await?;
        tokio::fs::rename(&staging, &destination).await?;
        Ok::<(), crate::Error>(())
    }
    .await;
    transaction.rollback().await?;
    if result.is_err() {
        // A partial snapshot never has the final directory name.
        tracing::warn!(path = %staging.display(), "Incomplete database snapshot retained for inspection");
    }
    result?;
    Ok(destination)
}

/// Keep at most three complete snapshot sets, taking a new one at most daily.
pub(crate) async fn maintain_backups(
    pool: &SqlitePool,
    settings_dir: &Path,
) -> crate::Result<()> {
    let parent = crate::state::db_backup::app_db_backup_dir_for(
        &directory(settings_dir).join("system.db"),
    )?;
    tokio::fs::create_dir_all(&parent).await?;
    let mut entries = tokio::fs::read_dir(&parent).await?;
    let mut snapshots = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_dir()
            || uuid::Uuid::parse_str(&entry.file_name().to_string_lossy())
                .is_err()
        {
            continue;
        }
        let marker = entry.path().join("complete");
        if let Ok(metadata) = tokio::fs::metadata(marker).await {
            snapshots.push((metadata.modified()?, entry.path()));
        }
    }
    snapshots.sort_by_key(|(modified, _)| *modified);
    let recent = snapshots.last().is_some_and(|(modified, _)| {
        std::time::SystemTime::now()
            .duration_since(*modified)
            .is_ok_and(|age| age < Duration::from_secs(24 * 60 * 60))
    });
    if !recent {
        let destination = backup(pool, settings_dir).await?;
        snapshots.push((std::time::SystemTime::now(), destination));
    }
    let remove = snapshots.len().saturating_sub(3);
    let resolved_parent = tokio::fs::canonicalize(&parent).await?;
    for (_, path) in snapshots.into_iter().take(remove) {
        let resolved = tokio::fs::canonicalize(&path).await?;
        if resolved.parent() != Some(resolved_parent.as_path()) {
            return Err(crate::ErrorKind::FSError("Snapshot retention target escaped the database backup directory".into()).into());
        }
        tokio::fs::remove_dir_all(resolved).await?;
    }
    Ok(())
}

/// All connections attach the same files before any collection can use them.
/// The configuration attachment is shared memory, not another durable copy of
/// the JSON configuration. A keeper connection preserves it for the pool's life.
pub(crate) async fn open(settings_dir: &Path) -> crate::Result<SqlitePool> {
    super::legacy_documents::initialize(settings_dir).await;
    let retired = crate::state::persistence::import::retired_database_domains(
        settings_dir,
    )
    .await?;
    let pool = open_with_retired(settings_dir, retired).await?;
    if let Err(error) = super::records_db::initialize(settings_dir, None).await
    {
        tracing::warn!(%error,"Optional daily history unavailable; core startup continues");
    }
    if !["app.db", "release/app.db", "beta/app.db"]
        .iter()
        .any(|path| settings_dir.join(path).is_file())
        || settings_dir.join("settings/bootstrap.json").is_file()
    {
        super::sync_documents::initialize(settings_dir, None).await;
    }
    Ok(pool)
}

async fn open_with_retired(
    settings_dir: &Path,
    retired: Vec<String>,
) -> crate::Result<SqlitePool> {
    let domains = Arc::new(
        DOMAINS
            .iter()
            .copied()
            .filter(|domain| {
                !matches!(
                    *domain,
                    "accounts" | "telemetry" | "media" | "records" | "sync"
                )
            })
            .filter(|domain| {
                !retired.iter().any(|name| name.as_str() == *domain)
            })
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    );
    let root = directory(settings_dir);
    tokio::fs::create_dir_all(&root).await?;
    if !root.join("system.db").is_file()
        && DOMAINS
            .iter()
            .any(|domain| root.join(format!("{domain}.db")).is_file())
    {
        return Err(crate::ErrorKind::FSError("The database coordinator is missing; existing domain files were preserved instead of reimported".into()).into());
    }
    let ownership = Arc::new(
        crate::state::db::acquire_domain_ownership(&root.join("system.db"))
            .await?,
    );
    let memory_name = format!(
        "file:/axolotl-configuration-{}?vfs=memdb",
        uuid::Uuid::new_v4()
    );
    let options = SqliteConnectOptions::new()
        .filename(directory(settings_dir).join("system.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Full)
        .busy_timeout(Duration::from_secs(30))
        .foreign_keys(true);
    let mut keeper = SqliteConnection::connect_with(&options).await?;
    let has_marker: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.sqlite_schema WHERE name = 'database_imports')").fetch_one(&mut keeper).await?;
    if has_marker {
        let completed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.database_imports WHERE source='app.db' AND completed=1)").fetch_one(&mut keeper).await?;
        if completed
            && domains.iter().any(|domain| {
                !directory(settings_dir)
                    .join(format!("{domain}.db"))
                    .is_file()
            })
        {
            return Err(crate::ErrorKind::FSError("A durable database domain is missing; restore the complete database set before continuing".into()).into());
        }
    }
    initialize_connection(&mut keeper, &root, &memory_name, true, &domains)
        .await?;
    let pool = SqlitePoolOptions::new()
        // Preserve the previous launcher pool budget: waiting SQLite writers
        // must not consume every slot needed by an active writer's readers.
        .max_connections(100)
        .min_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .after_connect(move |connection, _| {
            let root = root.clone();
            let memory_name = memory_name.clone();
            let ownership = ownership.clone();
            let domains = domains.clone();
            Box::pin(async move {
                let _ownership = ownership;
                initialize_connection(
                    connection,
                    &root,
                    &memory_name,
                    false,
                    &domains,
                )
                .await
            })
        })
        .connect_with(options)
        .await?;
    super::configuration::recover_and_load(&mut keeper, settings_dir).await?;
    keeper.close().await?;
    super::configuration::hydrate_settings(&pool, settings_dir).await?;
    Ok(pool)
}

async fn initialize_connection(
    connection: &mut SqliteConnection,
    root: &Path,
    memory_name: &str,
    durable: bool,
    domains: &[String],
) -> Result<(), sqlx::Error> {
    if durable {
        return attach_connection(connection, root, memory_name, true, domains)
            .await;
    }
    // Attaching schemas and compiling TEMP triggers can acquire read locks.
    // Reserve main first, exactly as ordinary writers do, so initialization
    // cannot retain an attached-file read lock while waiting for its writer.
    connection.execute("BEGIN IMMEDIATE").await?;
    match attach_connection(connection, root, memory_name, false, domains).await
    {
        Ok(()) => connection.execute("COMMIT").await.map(|_| ()),
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            Err(error)
        }
    }
}

async fn attach_connection(
    connection: &mut SqliteConnection,
    root: &Path,
    memory_name: &str,
    durable: bool,
    domains: &[String],
) -> Result<(), sqlx::Error> {
    // memdb shares the volatile projection without SQLite's shared-cache
    // schema locks, which can invert the attached files' writer lock order.
    sqlx::query("ATTACH DATABASE ? AS configuration")
        .bind(memory_name)
        .execute(&mut *connection)
        .await?;
    for domain in domains {
        sqlx::query(sqlx::AssertSqlSafe(
            format!("ATTACH DATABASE ? AS {}", quote(domain)).as_str(),
        ))
        .bind(root.join(format!("{domain}.db")).to_string_lossy().as_ref())
        .execute(&mut *connection)
        .await?;
        if durable {
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.journal_mode = DELETE", quote(domain))
                        .as_str(),
                ))
                .await?;
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.synchronous = FULL", quote(domain))
                        .as_str(),
                ))
                .await?;
        }
    }
    if !durable {
        return create_schema(connection, false).await;
    }
    connection.execute("BEGIN IMMEDIATE").await?;
    let result = create_schema(connection, true).await;
    match result {
        Ok(()) => connection.execute("COMMIT").await.map(|_| ()),
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            Err(error)
        }
    }
}

async fn create_schema(
    connection: &mut SqliteConnection,
    durable: bool,
) -> Result<(), sqlx::Error> {
    let attached = attached_domains(connection).await?;
    if durable {
        super::sync_documents::install(connection).await?;
        connection.execute("CREATE TABLE IF NOT EXISTS main.database_imports (source TEXT PRIMARY KEY NOT NULL, completed INTEGER NOT NULL)").await?;
        for table in &baseline().tables {
            if !attached.contains(&table.domain) {
                continue;
            }
            let body = &table.sql[table.sql.find('(').expect("table body")..];
            connection
                .execute(sqlx::AssertSqlSafe(
                    format!(
                        "CREATE TABLE IF NOT EXISTS {} {body}",
                        qualified(table)
                    )
                    .as_str(),
                ))
                .await?;
            for seed in &table.seeds {
                connection
                    .execute(sqlx::AssertSqlSafe(seed.as_str()))
                    .await?;
            }
        }
        for domain in DOMAINS {
            if !attached.contains(*domain) {
                continue;
            }
            connection.execute(sqlx::AssertSqlSafe(format!("CREATE TABLE IF NOT EXISTS {}.extension_records (namespace TEXT NOT NULL, id TEXT NOT NULL, format TEXT NOT NULL, payload BLOB NOT NULL, PRIMARY KEY(namespace,id))", quote(domain)).as_str())).await?;
        }
        connection
            .execute("UPDATE configuration.settings SET version = 3")
            .await?;
    }
    crate::state::instances::adapters::sqlite::manifest_rows::install(
        connection, durable,
    )
    .await?;
    for object in &baseline().objects {
        if object.name == "resource_favorite_links" {
            continue;
        }
        // Views are instance/storage-only projections. Table-owned indexes
        // and triggers disappear together with their retired domain.
        if let Some(table) = baseline()
            .tables
            .iter()
            .find(|table| table.name == object.table)
            && !attached.contains(&table.domain)
        {
            continue;
        }
        let sql = match object.kind.as_str() {
            "index" if !durable => continue,
            "index" => {
                let table = baseline()
                    .tables
                    .iter()
                    .find(|table| table.name == object.table)
                    .expect("index table");
                let on = object.sql.find("ON ").expect("index ON");
                let unique = if object.sql.starts_with("CREATE UNIQUE") {
                    "UNIQUE "
                } else {
                    ""
                };
                format!(
                    "CREATE {unique}INDEX IF NOT EXISTS {}.{}{}",
                    quote(&table.domain),
                    quote(&object.name),
                    format!(" {}", &object.sql[on..])
                )
            }
            "view" => object.sql.replacen(
                "CREATE VIEW",
                "CREATE TEMP VIEW IF NOT EXISTS",
                1,
            ),
            "trigger" => {
                let table = baseline()
                    .tables
                    .iter()
                    .find(|table| table.name == object.table)
                    .expect("trigger table");
                object
                    .sql
                    .replacen(
                        "CREATE TRIGGER",
                        "CREATE TEMP TRIGGER IF NOT EXISTS",
                        1,
                    )
                    .replacen(
                        &format!("ON {}", object.table),
                        &format!("ON {}", qualified(table)),
                        1,
                    )
            }
            _ => continue,
        };
        connection
            .execute(sqlx::AssertSqlSafe(sql.as_str()))
            .await?;
    }
    for table in &baseline().tables {
        if !attached.contains(&table.domain) {
            continue;
        }
        for (index, foreign) in table.foreign_keys.iter().enumerate() {
            let parent = baseline()
                .tables
                .iter()
                .find(|table| table.name == foreign.parent)
                .expect("foreign parent");
            if parent.domain == table.domain {
                continue;
            }
            if !attached.contains(&parent.domain) {
                continue;
            }
            let present = foreign
                .columns
                .iter()
                .map(|column| {
                    format!("NEW.{} IS NOT NULL", quote(&column.child))
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matches_new = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "{} = NEW.{}",
                        quote(&column.parent),
                        quote(&column.child)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            let matches_old = foreign
                .columns
                .iter()
                .map(|column| {
                    format!(
                        "{} = OLD.{}",
                        quote(&column.child),
                        quote(&column.parent)
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");
            for event in ["INSERT", "UPDATE"] {
                let name = quote(&format!(
                    "domain_{}_{}_{}",
                    table.name, index, event
                ));
                let sql = format!(
                    "CREATE TEMP TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {} WHEN {present} AND NOT EXISTS (SELECT 1 FROM {} WHERE {matches_new}) BEGIN SELECT RAISE(ABORT, 'cross-domain foreign key'); END",
                    qualified(table),
                    qualified(parent)
                );
                connection
                    .execute(sqlx::AssertSqlSafe(sql.as_str()))
                    .await?;
            }
            for (event, action) in [
                ("DELETE", &foreign.on_delete),
                ("UPDATE", &foreign.on_update),
            ] {
                let assignments = foreign
                    .columns
                    .iter()
                    .map(|column| {
                        format!(
                            "{} = NEW.{}",
                            quote(&column.child),
                            quote(&column.parent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let nulls = foreign
                    .columns
                    .iter()
                    .map(|column| format!("{} = NULL", quote(&column.child)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let effect = match action.as_str() {
                    "CASCADE" if event == "DELETE" => format!(
                        "DELETE FROM {} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    "CASCADE" => format!(
                        "UPDATE {} SET {assignments} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    "SET NULL" => format!(
                        "UPDATE {} SET {nulls} WHERE {matches_old};",
                        quote(&table.name)
                    ),
                    _ => format!(
                        "SELECT RAISE(ABORT, 'cross-domain foreign key') WHERE EXISTS (SELECT 1 FROM {} WHERE {matches_old});",
                        qualified(table)
                    ),
                };
                let name = quote(&format!(
                    "domain_{}_{}_parent_{}",
                    table.name, index, event
                ));
                let changed = foreign
                    .columns
                    .iter()
                    .map(|column| {
                        format!(
                            "NEW.{} IS NOT OLD.{}",
                            quote(&column.parent),
                            quote(&column.parent)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let condition = if event == "UPDATE" {
                    format!(" WHEN {changed}")
                } else {
                    String::new()
                };
                connection.execute(sqlx::AssertSqlSafe(format!("CREATE TEMP TRIGGER IF NOT EXISTS {name} BEFORE {event} ON {}{condition} BEGIN {effect} END", qualified(parent)).as_str())).await?;
            }
        }
    }
    let owned = baseline()
        .tables
        .iter()
        .filter(|table| attached.contains(&table.domain))
        .map(|table| {
            (
                table.domain.as_str(),
                table.name.as_str(),
                table.name == "instances"
                    || table
                        .columns
                        .iter()
                        .any(|column| column == "instance_id"),
            )
        })
        .collect::<Vec<_>>();
    super::compatibility::install(connection, &owned).await?;
    super::configuration::install_tracking(connection, durable).await?;
    Ok(())
}

/// Copy a fully migrated legacy snapshot exactly once. Configuration is only
/// copied into memory and must be handed to JSON before import is marked done.
pub(crate) async fn import(
    pool: &SqlitePool,
    source: &Path,
    settings_dir: &Path,
) -> crate::Result<()> {
    let completed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM main.database_imports WHERE source = 'app.db' AND completed = 1)").fetch_one(pool).await?;
    if completed {
        return Ok(());
    }
    let mut connection = pool.acquire().await?;
    sqlx::query("ATTACH DATABASE ? AS legacy_import")
        .bind(source.to_string_lossy().as_ref())
        .execute(&mut *connection)
        .await?;
    let triggers: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM temp.sqlite_schema WHERE type = 'trigger'",
    )
    .fetch_all(&mut *connection)
    .await?;
    for trigger in triggers {
        connection
            .execute(sqlx::AssertSqlSafe(
                format!("DROP TRIGGER temp.{}", quote(&trigger)).as_str(),
            ))
            .await?;
    }
    // The import attachment is the tenth (SQLite's default maximum).
    connection.execute("PRAGMA foreign_keys = OFF").await?;
    connection.execute("BEGIN IMMEDIATE").await?;
    let result = async {
        let attached = attached_domains(&mut connection).await?;
        for table in &baseline().tables {
            if !attached.contains(&table.domain) { continue; }
            if table.name == "cache" { continue; }
            let columns = table.columns.iter().map(|column| quote(column)).collect::<Vec<_>>().join(", ");
            connection.execute(sqlx::AssertSqlSafe(format!("DELETE FROM {}", qualified(table)).as_str())).await?;
            connection.execute(sqlx::AssertSqlSafe(format!("INSERT INTO {} ({columns}) SELECT {columns} FROM legacy_import.{}", qualified(table), quote(&table.name)).as_str())).await?;
        }
        Ok::<(), sqlx::Error>(())
    }.await;
    match result {
        Ok(()) => {
            connection.execute("COMMIT").await?;
        }
        Err(error) => {
            let _ = connection.execute("ROLLBACK").await;
            let _ = connection.execute("DETACH DATABASE legacy_import").await;
            return Err(error.into());
        }
    }
    connection.execute("DETACH DATABASE legacy_import").await?;
    connection.execute("PRAGMA foreign_keys = ON").await?;
    create_schema(&mut connection, false).await?;
    super::configuration::recover_and_load(&mut connection, settings_dir)
        .await?;
    drop(connection);
    crate::state::Settings::migrate_legacy_row(&super::Handle::new(
        pool.clone(),
    ))
    .await?;
    if !crate::state::documents::hand_over(pool, settings_dir).await {
        return Err(crate::ErrorKind::FSError("Configuration handover did not finish; the legacy database is retained for retry".into()).into());
    }
    super::configuration::hand_over(pool).await?;
    super::configuration::hydrate_settings(pool, settings_dir).await?;
    sqlx::query("INSERT INTO main.database_imports (source, completed) VALUES ('app.db', 1) ON CONFLICT(source) DO UPDATE SET completed = 1").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn retired_domains_are_not_opened_recreated_or_reimported() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO main.database_imports VALUES('app.db',1)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        for domain in ["accounts", "telemetry", "media", "sync"] {
            let document = persistence
                .document(format!("state/{domain}.json"), "test", 1)
                .unwrap();
            crate::state::persistence::import::Import::new(
                &persistence,
                domain,
                document,
            )
            .unwrap()
            .run(|| async { Ok(serde_json::Map::new()) }, |_| Ok(()))
            .await
            .unwrap();
            tokio::fs::write(
                directory(temporary.path()).join(format!("{domain}.db")),
                b"obsolete corrupt database",
            )
            .await
            .unwrap();
        }
        let pool = open(temporary.path()).await.unwrap();
        let mut first = pool.acquire().await.unwrap();
        let mut second = pool.acquire().await.unwrap();
        for connection in [&mut first, &mut second] {
            let attached = attached_domains(connection).await.unwrap();
            for domain in ["accounts", "telemetry", "media", "sync"] {
                assert!(!attached.contains(domain));
            }
            for domain in
                ["instances", "storage", "operations", "configuration"]
            {
                assert!(attached.contains(domain));
            }
            assert_eq!(cross_domain_issues(connection).await.unwrap(), 0);
            sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('retired-test','path','installed','none','test',0,0)").execute(&mut **connection).await.unwrap();
            sqlx::query("DELETE FROM instances WHERE id='retired-test'")
                .execute(&mut **connection)
                .await
                .unwrap();
        }
        drop(first);
        drop(second);
        assert!(ensure_cleanup_supported(&pool).await.is_err());
        for domain in ["accounts", "telemetry", "media", "sync"] {
            assert_eq!(
                tokio::fs::read(
                    directory(temporary.path()).join(format!("{domain}.db"))
                )
                .await
                .unwrap(),
                b"obsolete corrupt database"
            );
        }
        pool.close().await;
    }

    #[tokio::test]
    async fn a_retired_missing_domain_stays_missing_when_legacy_import_retries()
    {
        let temporary = tempfile::tempdir().unwrap();
        let legacy_path = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&legacy_path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("INSERT INTO telemetry_identity(id,installation_id) VALUES(0,'old-identity')").execute(&legacy).await.unwrap();
        legacy.close().await;
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let document = persistence
            .document("state/telemetry.json", "test", 1)
            .unwrap();
        crate::state::persistence::import::Import::new(
            &persistence,
            "telemetry",
            document,
        )
        .unwrap()
        .run(|| async { Ok(serde_json::Map::new()) }, |_| Ok(()))
        .await
        .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &legacy_path, temporary.path()).await.unwrap();
        assert!(!directory(temporary.path()).join("telemetry.db").exists());
        pool.close().await;
    }

    #[tokio::test]
    async fn unreadable_handover_does_not_make_retired_sql_data_available() {
        let temporary = tempfile::tempdir().unwrap();
        let receipts = temporary.path().join("state/persistence/imports");
        tokio::fs::create_dir_all(&receipts).await.unwrap();
        tokio::fs::write(receipts.join("accounts.json"), b"broken")
            .await
            .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        assert!(
            !attached_domains(&mut pool.acquire().await.unwrap())
                .await
                .unwrap()
                .contains("accounts")
        );
        assert!(!directory(temporary.path()).join("accounts.db").exists());
        assert_eq!(
            tokio::fs::read(receipts.join("accounts.json"))
                .await
                .unwrap(),
            b"broken"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn snapshot_keeps_retirement_receipt_and_document_instead_of_old_database()
     {
        let temporary = tempfile::tempdir().unwrap();
        let persistence =
            crate::state::persistence::Persistence::new(temporary.path());
        let document = persistence
            .document("state/accounts.json", "test", 1)
            .unwrap();
        crate::state::persistence::import::Import::new(
            &persistence,
            "accounts",
            document.clone(),
        )
        .unwrap()
        .run(
            || async {
                Ok(serde_json::Map::from_iter([(
                    "metadata".into(),
                    serde_json::json!("account"),
                )]))
            },
            |_| Ok(()),
        )
        .await
        .unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let snapshot = backup(&pool, temporary.path()).await.unwrap();
        assert!(!snapshot.join("accounts.db").exists());
        assert!(
            snapshot
                .join("state/persistence/imports/accounts.json")
                .is_file()
        );
        assert_eq!(
            tokio::fs::read(snapshot.join("state/accounts.json"))
                .await
                .unwrap(),
            tokio::fs::read(document.path()).await.unwrap()
        );
        assert!(snapshot.join("complete").is_file());
        pool.close().await;
    }

    #[tokio::test]
    async fn every_connection_attaches_domains_without_application_migrations()
    {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let mut first = pool.acquire().await.unwrap();
        let mut second = pool.acquire().await.unwrap();
        for connection in [&mut first, &mut second] {
            let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pragma_database_list WHERE name IN ('instances', 'accounts', 'sync', 'media', 'operations', 'storage', 'records', 'telemetry', 'configuration')").fetch_one(&mut **connection).await.unwrap();
            assert_eq!(count, 4);
            let migrations: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM main.sqlite_schema WHERE name = '_sqlx_migrations'").fetch_one(&mut **connection).await.unwrap();
            assert_eq!(migrations, 0);
            let version: i64 =
                sqlx::query_scalar("SELECT version FROM settings WHERE id = 0")
                    .fetch_one(&mut **connection)
                    .await
                    .unwrap();
            assert_eq!(version, 3);
            for schema in std::iter::once("main").chain(
                DOMAINS.iter().copied().filter(|domain| {
                    !matches!(
                        *domain,
                        "accounts" | "telemetry" | "media" | "records" | "sync"
                    )
                }),
            ) {
                let mode: String = sqlx::query_scalar(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.journal_mode", quote(schema)),
                ))
                .fetch_one(&mut **connection)
                .await
                .unwrap();
                let synchronous: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(
                    format!("PRAGMA {}.synchronous", quote(schema)),
                ))
                .fetch_one(&mut **connection)
                .await
                .unwrap();
                assert_eq!(mode, "delete", "{schema}");
                assert_eq!(synchronous, 2, "{schema}");
            }
        }
        sqlx::query("UPDATE settings SET default_page = 'test'")
            .execute(&mut *first)
            .await
            .unwrap();
        let value: String = sqlx::query_scalar(
            "SELECT default_page FROM settings WHERE id = 0",
        )
        .fetch_one(&mut *second)
        .await
        .unwrap();
        assert_eq!(value, "test");
        drop(first);
        drop(second);
        pool.close().await;
        assert!(
            !directory(temporary.path())
                .join("configuration.db")
                .exists()
        );
        for domain in DOMAINS.iter().filter(|domain| {
            !matches!(
                **domain,
                "accounts" | "telemetry" | "media" | "records" | "sync"
            )
        }) {
            assert!(
                directory(temporary.path())
                    .join(format!("{domain}.db"))
                    .is_file()
            );
        }
    }

    #[tokio::test]
    async fn cross_domain_foreign_keys_and_rollback_are_preserved() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('instance','path','installed','none','test',0,0)").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO instance_sync_preferences(instance_id, feature, enabled) VALUES ('instance','screenshots',1)").execute(&pool).await.unwrap();
        assert!(sqlx::query("INSERT INTO instance_sync_preferences(instance_id, feature, enabled) VALUES ('missing','screenshots',1)").execute(&pool).await.is_err());
        let mut transaction = pool.begin().await.unwrap();
        sqlx::query("DELETE FROM instances WHERE id='instance'")
            .execute(&mut *transaction)
            .await
            .unwrap();
        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_sync_preferences WHERE instance_id='instance'").fetch_one(&mut *transaction).await.unwrap();
        assert_eq!(remaining, 0);
        transaction.rollback().await.unwrap();
        let restored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instance_sync_preferences WHERE instance_id='instance'").fetch_one(&pool).await.unwrap();
        assert_eq!(restored, 1);
        pool.close().await;
    }

    #[tokio::test]
    async fn composite_foreign_keys_match_the_complete_parent_key() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        for id in ["first", "second"] {
            sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,'installed','none','test',0,0)")
                .bind(id).bind(id).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO instance_sync_preferences VALUES('first','screenshots',1),('second','game_options',1)").execute(&pool).await.unwrap();
        use super::super::instance_sync_checkpoints::{
            CheckpointKey, CheckpointRecord, CheckpointStatus,
            InstanceSyncCheckpoints,
        };
        let handle = super::super::Handle::new(pool.clone());
        let record = CheckpointRecord {
            expected_sha1: "hash".into(),
            merge_base: None,
            source_revision: 0,
            status: CheckpointStatus::Ready,
            link_mode: None,
        };
        let key = |feature: &str| CheckpointKey {
            instance_id: "first".into(),
            feature: feature.into(),
            variant: "default".into(),
        };
        assert!(
            InstanceSyncCheckpoints::set(
                &handle,
                &key("game_options"),
                Some(&record)
            )
            .await
            .is_err()
        );
        InstanceSyncCheckpoints::set(
            &handle,
            &key("screenshots"),
            Some(&record),
        )
        .await
        .unwrap();
        let mut unit = super::super::atom(&handle).await.unwrap();
        sqlx::query(
            "DELETE FROM instance_sync_preferences WHERE instance_id='second'",
        )
        .execute(unit.connection())
        .await
        .unwrap();
        unit.commit().await.unwrap();
        assert!(
            InstanceSyncCheckpoints::get(&handle, &key("screenshots"))
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn a_new_connection_waits_for_the_writer_before_attaching_domains() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let release = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            transaction.rollback().await.unwrap();
        };
        let acquire = async {
            let mut reader = pool.acquire().await.unwrap();
            let count: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM instances")
                    .fetch_one(&mut *reader)
                    .await
                    .unwrap();
            assert_eq!(count, 0);
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(release, acquire);
        })
        .await
        .expect("initialization must resume when the existing writer finishes");
    }

    #[tokio::test]
    async fn legacy_import_is_once_only_and_hands_configuration_to_documents() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query(
            "UPDATE settings SET default_page = 'library', version = 3",
        )
        .execute(&legacy)
        .await
        .unwrap();
        sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES ('instance','path','installed','none','original',0,0)").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        let appearance: serde_json::Value = serde_json::from_slice(
            &tokio::fs::read(temporary.path().join("settings/appearance.json"))
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(appearance["data"]["default_page"], "Library");
        let name: String = sqlx::query_scalar(
            "SELECT name FROM instances WHERE id='instance'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(name, "original");
        sqlx::query("UPDATE instances SET name='edited' WHERE id='instance'")
            .execute(&pool)
            .await
            .unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        let name: String = sqlx::query_scalar(
            "SELECT name FROM instances WHERE id='instance'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(name, "edited");
        pool.close().await;
    }

    #[tokio::test]
    async fn legacy_row_migration_does_not_migrate_existing_document_hooks_again()
     {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("settings");
        tokio::fs::create_dir(&directory).await.unwrap();
        let original = br#"{"schema_version":2,"data":{"hooks":{"wrapper":"\"already quoted command\""}}}"#;
        tokio::fs::write(directory.join("game.json"), original)
            .await
            .unwrap();
        crate::state::documents::settings::identify(temporary.path());
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("UPDATE settings SET version=1,hook_wrapper='legacy unquoted command'").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        assert_eq!(
            tokio::fs::read(directory.join("game.json")).await.unwrap(),
            original
        );
        let version: i64 =
            sqlx::query_scalar("SELECT version FROM settings WHERE id=0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(version, 3);
    }

    #[tokio::test]
    async fn imported_proxy_and_provider_credentials_live_in_json_after_reopen()
    {
        use crate::state::{curseforge_settings, proxy_settings};
        use crate::util::proxy::{ProxyConfig, ProxyMode};
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("legacy.db");
        let legacy = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&source)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&legacy).await.unwrap();
        sqlx::query("UPDATE settings SET proxy_mode='custom',proxy_url='http://proxy.test:8080',proxy_username='test-user',proxy_password='test-password',curseforge_api_key='test-import-key'").execute(&legacy).await.unwrap();
        legacy.close().await;
        let pool = open(temporary.path()).await.unwrap();
        import(&pool, &source, temporary.path()).await.unwrap();
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        let proxy = proxy_settings::get(&pool).await.unwrap();
        assert_eq!(proxy.mode, ProxyMode::Custom);
        assert_eq!(proxy.password, "test-password");
        assert_eq!(
            curseforge_settings::get(&pool).await.unwrap().as_deref(),
            Some("test-import-key")
        );
        proxy_settings::set(
            &pool,
            &ProxyConfig {
                mode: ProxyMode::Custom,
                url: "http://other.test:9090".into(),
                username: "changed-user".into(),
                password: "changed-password".into(),
            },
        )
        .await
        .unwrap();
        curseforge_settings::set(&pool, Some("changed-key"))
            .await
            .unwrap();
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        let proxy = proxy_settings::get(&pool).await.unwrap();
        assert_eq!(proxy.url, "http://other.test:9090");
        assert_eq!(proxy.username, "changed-user");
        assert_eq!(proxy.password, "changed-password");
        assert_eq!(
            curseforge_settings::get(&pool).await.unwrap().as_deref(),
            Some("changed-key")
        );
        assert!(temporary.path().join("settings/proxy.json").is_file());
        assert!(
            temporary
                .path()
                .join("settings/content_providers.json")
                .is_file()
        );
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_instance_and_preference_writes_reserve_domains_in_order()
     {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let write = |prefix: &'static str| {
            let pool = pool.clone();
            async move {
                for index in 0..3 {
                    let id = format!("{prefix}-{index}");
                    let mut transaction =
                        pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
                    sqlx::query("INSERT INTO instances(id,path,install_stage,launcher_feature_version,name,created,modified) VALUES(?,?,'installed','none','concurrent',0,0)").bind(&id).bind(&id).execute(&mut *transaction).await.unwrap();
                    sqlx::query("INSERT INTO instance_sync_preferences VALUES(?,'screenshots',1)").bind(&id).execute(&mut *transaction).await.unwrap();
                    super::super::configuration::commit(transaction, &pool)
                        .await
                        .unwrap();
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(20), async {
            tokio::join!(
                write("first"),
                write("second"),
                write("third"),
                write("fourth")
            );
        })
        .await
        .expect("domain writers must not deadlock");
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 12);
        pool.close().await;
        drop(pool);
        let pool = open(temporary.path()).await.unwrap();
        let preferences: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM instance_sync_preferences",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(preferences, 12);
    }

    #[tokio::test]
    async fn new_connection_resumes_after_a_configuration_writer_commits() {
        let temporary = tempfile::tempdir().unwrap();
        let pool = open(temporary.path()).await.unwrap();
        let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("UPDATE settings SET default_page='instances'")
            .execute(&mut *transaction)
            .await
            .unwrap();
        let commit = async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            transaction.commit().await.unwrap();
        };
        let read = async {
            let mut reader = pool.acquire().await.unwrap();
            sqlx::query_scalar::<_, String>(
                "SELECT default_page FROM settings WHERE id=0",
            )
            .fetch_one(&mut *reader)
            .await
            .unwrap()
        };
        let ((), value) = tokio::time::timeout(Duration::from_secs(5), async { tokio::join!(commit, read) }).await.expect("new connections must resume after the configuration transaction commits");
        assert_eq!(value, "instances");
    }
}

/// Build a disposable read-only projection from validated recovery copies.
/// Source database files are never opened for mutation by this path.
pub(crate) async fn open_recovery(root: &Path) -> crate::Result<SqlitePool> {
    let snapshots = crate::state::instances::manifest::list(root).await?;
    if snapshots.is_empty() {
        return Err(crate::ErrorKind::FSError(
            "No complete instance recovery copies are available".into(),
        )
        .into());
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect_with(
            SqliteConnectOptions::new()
                .in_memory(true)
                .foreign_keys(false),
        )
        .await?;
    let mut connection = pool.acquire().await?;
    connection
        .execute("ATTACH DATABASE ':memory:' AS configuration")
        .await?;
    for domain in ["instances", "storage", "operations", "sync"] {
        connection
            .execute(sqlx::AssertSqlSafe(format!(
                "ATTACH DATABASE ':memory:' AS {}",
                quote(domain)
            )))
            .await?;
    }
    create_schema(&mut connection, true).await?;
    let triggers: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM temp.sqlite_schema WHERE type='trigger'",
    )
    .fetch_all(&mut *connection)
    .await?;
    for trigger in triggers {
        connection
            .execute(sqlx::AssertSqlSafe(format!(
                "DROP TRIGGER temp.{}",
                quote(&trigger)
            )))
            .await?;
    }
    let mut imported = 0usize;
    for snapshot in snapshots {
        let id = snapshot.metadata.instance.id.clone();
        let mut transaction = connection.begin().await?;
        match import_recovery_snapshot(&mut transaction, &snapshot).await {
            Ok(()) => {
                transaction.commit().await?;
                imported += 1;
            }
            Err(error) => {
                transaction.rollback().await?;
                tracing::warn!(instance_id=%id,%error,"Unsupported or incomplete recovery copy retained; skipping this instance");
            }
        }
    }
    if imported == 0 {
        return Err(crate::ErrorKind::FSError(
            "No supported complete instance recovery copies are available"
                .into(),
        )
        .into());
    }
    if let Err(error) =
        super::configuration::recover_and_load(&mut connection, root).await
    {
        tracing::warn!(%error,"Optional preference projection is unavailable in instance recovery mode");
    }
    connection.execute("PRAGMA query_only=ON").await?;
    drop(connection);
    super::recovery_mode::register(&pool, root);
    Ok(pool)
}

async fn import_recovery_snapshot(
    connection: &mut SqliteConnection,
    snapshot: &crate::state::instances::manifest::RecoverySnapshot,
) -> crate::Result<()> {
    let id = &snapshot.metadata.instance.id;
    // Validate references against this copy before importing into a projection
    // that may already contain other recovered instances.
    let ids = |table: &str| {
        snapshot
            .relationships
            .get(table)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|row| row.get("id").and_then(serde_json::Value::as_str))
            .collect::<HashSet<_>>()
    };
    let sets = ids("instance_content_sets");
    let files = ids("instance_files");
    let entries = ids("instance_content_entries");
    let members = ids("instance_pack_members");
    for (table, rows) in &snapshot.relationships {
        let Some(rows) = rows.as_array() else {
            continue;
        };
        for row in rows {
            for (key, targets) in [
                ("content_set_id", &sets),
                ("content_entry_id", &entries),
                ("parent_entry_id", &entries),
                ("child_entry_id", &entries),
                ("entry_id", &entries),
                ("file_id", &files),
                ("pack_member_id", &members),
            ] {
                if let Some(target) =
                    row.get(key).and_then(serde_json::Value::as_str)
                    && !targets.contains(target)
                {
                    return Err(crate::ErrorKind::InputError(format!("Recovery relationship {table}.{key} escapes its instance copy")).into());
                }
            }
        }
    }

    for table in &baseline().tables {
        let Some(rows) = snapshot
            .relationships
            .get(&table.name)
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        if table.domain != "instances" {
            continue;
        }
        for raw in rows {
            let object = raw.as_object().ok_or_else(|| {
                crate::ErrorKind::InputError(
                    "Recovery row must be an object".into(),
                )
            })?;
            if object
                .get("instance_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|owner| owner != id)
                || (table.name == "instances"
                    && object.get("id").and_then(serde_json::Value::as_str)
                        != Some(id.as_str()))
            {
                return Err(crate::ErrorKind::InputError(
                    "Recovery row belongs to another instance".into(),
                )
                .into());
            }
            for key in [
                "content_set_id",
                "content_entry_id",
                "parent_entry_id",
                "child_entry_id",
                "entry_id",
                "file_id",
                "pack_member_id",
            ] {
                let Some(target) =
                    object.get(key).and_then(serde_json::Value::as_str)
                else {
                    continue;
                };
                let ownership = match key {
                    "content_set_id" => {
                        "SELECT instance_id FROM instance_content_sets WHERE id=?"
                    }
                    "file_id" => {
                        "SELECT instance_id FROM instance_files WHERE id=?"
                    }
                    "pack_member_id" => {
                        "SELECT content.instance_id FROM instance_pack_members member JOIN instance_content_sets content ON content.id=member.content_set_id WHERE member.id=?"
                    }
                    _ => {
                        "SELECT instance_id FROM instance_content_entries WHERE id=?"
                    }
                };
                let owner: Option<String> =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(ownership))
                        .bind(target)
                        .fetch_optional(&mut *connection)
                        .await?;
                if owner.as_deref().is_some_and(|owner| owner != id) {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery relationship points into another instance"
                            .into(),
                    )
                    .into());
                }
            }
            if let Some(key) =
                object.get("id").and_then(serde_json::Value::as_str)
                && table.columns.iter().any(|column| column == "instance_id")
            {
                let owner: Option<String> =
                    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT instance_id FROM {} WHERE id=?",
                        qualified(table)
                    )))
                    .bind(key)
                    .fetch_optional(&mut *connection)
                    .await?;
                if owner.as_deref().is_some_and(|owner| owner != id) {
                    return Err(crate::ErrorKind::InputError(
                        "Recovery identity collides with another instance"
                            .into(),
                    )
                    .into());
                }
            }
            let columns = table
                .columns
                .iter()
                .filter(|name| object.contains_key(*name))
                .collect::<Vec<_>>();
            if columns.is_empty() {
                return Err(crate::ErrorKind::InputError(
                    "Recovery row has no supported columns".into(),
                )
                .into());
            }
            let shared = matches!(
                table.name.as_str(),
                "resources"
                    | "resource_source_projects"
                    | "resource_source_types"
                    | "resource_aliases"
                    | "resource_identity_evidence"
            );
            let conflict = if shared {
                " ON CONFLICT DO NOTHING"
            } else {
                ""
            };
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({}){conflict}",
                qualified(table),
                columns
                    .iter()
                    .map(|name| quote(name))
                    .collect::<Vec<_>>()
                    .join(","),
                vec!["?"; columns.len()].join(",")
            );
            let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
            for name in columns {
                let value = &object[name];
                query = match value {
                    serde_json::Value::Null => query.bind(None::<String>),
                    serde_json::Value::Number(value) if value.is_i64() => {
                        query.bind(value.as_i64())
                    }
                    serde_json::Value::Number(value) => {
                        query.bind(value.as_f64())
                    }
                    serde_json::Value::String(value) => {
                        query.bind(value.clone())
                    }
                    serde_json::Value::Object(value)
                        if value.contains_key("sqlite_blob") =>
                    {
                        query.bind(serde_json::from_value::<Vec<u8>>(
                            value["sqlite_blob"].clone(),
                        )?)
                    }
                    _ => {
                        return Err(crate::ErrorKind::InputError(
                            "Unsupported recovery SQL value".into(),
                        )
                        .into());
                    }
                };
            }
            query.execute(&mut *connection).await?;
        }
    }
    if super::compatibility::unsupported_on(connection)
        .await?
        .contains(id)
    {
        return Err(crate::ErrorKind::InputError(
            "Recovery copy contains unsupported persistent data".into(),
        )
        .into());
    }
    let record=crate::state::instances::adapters::sqlite::instance_rows::recovery_metadata_on(id,connection).await?.ok_or_else(||crate::ErrorKind::InputError("Recovery identity is missing".into()))?;
    let metadata = &snapshot.metadata;
    if record.instance.id != *id
        || record.instance.path != metadata.instance.path
        || record.applied_content_set.id != metadata.applied_content_set.id
        || record.applied_content_set.revision
            != metadata.applied_content_set.revision
        || record.applied_content_set.instance_id != *id
    {
        return Err(crate::ErrorKind::InputError("Recovery descriptor disagrees with authoritative relationship rows".into()).into());
    }
    crate::state::instances::adapters::sqlite::loader_component_rows::recovery_components_on(id,connection).await?;
    Ok(())
}
